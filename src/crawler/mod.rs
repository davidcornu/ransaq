//! Connecting logic between [`saq`](saq) and [`db`](db) to actually
//! perform a crawl.

use crate::db::{self, DbSerialize, ProductUpsertFields};
use crate::saq::{self, ExtractedProduct};
use color_eyre::{Report, Result};
use futures_util::future::join_all;

/// Iterates through the entire product catalog page by page, fetches
/// and parses each product page, and inserts the relevant data into
/// the database.
///
/// Catalog pages are fetched serially, each yielding a list of product
/// page URLs. These are then handed to a pool of tasks to be fetched
/// in parallel.
///
/// Task coordination and backpressure is handled via [`async_channel::bounded`](async_channel::bounded).
pub async fn crawl() -> Result<()> {
    let client = saq::Client::new()?;
    let db = db::Client::new_from_env().await?;

    let (send, receive) = async_channel::bounded(8);

    let page_client = client.clone();
    let page_task = tokio::spawn(async move {
        let mut page_number = 1;
        loop {
            match page_client.page(page_number).await {
                Ok(Some(page)) => {
                    for product in page {
                        if let Err(err) = send.send(product).await {
                            return Err(Report::from(err));
                        }
                    }
                    page_number += 1;
                }
                // We've hit the last page
                Ok(None) => {
                    send.close();
                    return Ok(());
                }
                // There was an error fetching the current page
                Err(err) => {
                    send.close();
                    return Err(err);
                }
            }
        }
    });

    let product_tasks = (0..8)
        .into_iter()
        .map(|_| {
            let client = client.clone();
            let db = db.clone();
            let receive = receive.clone();

            tokio::spawn(async move {
                loop {
                    match receive.recv().await {
                        Ok(product) => {
                            let extracted = match client.product(&product).await {
                                Ok(value) => value,
                                Err(err) => {
                                    receive.close();
                                    return Err(err);
                                }
                            };

                            if let Err(err) = persist_product(&db, extracted).await {
                                receive.close();
                                return Err(err);
                            } else {
                                continue;
                            }
                        }
                        // The channel is closed
                        Err(_) => {
                            return Ok(());
                        }
                    }
                }
            })
        })
        .collect::<Vec<_>>();

    page_task.await??;

    for join_result in join_all(product_tasks).await {
        join_result??;
    }

    Ok(())
}

/// Ensures the given [`ExtractedProduct`](crate::saq::ExtractedProduct) is present
/// and up to date in the database, updating all the necessary relations along
/// the way.
async fn persist_product(db: &db::Client, product: ExtractedProduct) -> Result<()> {
    let producer_id = match &product.detailed_info.producer {
        Some(name) => Some(db.upsert_producer(name).await?),
        None => None,
    };

    let promoting_agent_id = match &product.detailed_info.promoting_agent {
        Some(name) => Some(db.upsert_promoting_agent(name).await?),
        None => None,
    };

    let color_id = match &product.detailed_info.color {
        Some(name) => Some(db.upsert_color(name).await?),
        None => None,
    };

    let region_id = match &product.detailed_info.region {
        Some(name) => Some(db.upsert_region(name).await?),
        None => None,
    };

    let country_id = match &product.detailed_info.country {
        Some(name) => Some(db.upsert_country(name).await?),
        None => None,
    };

    let regulated_designation_id = match &product.detailed_info.regulated_designation {
        Some(name) => Some(db.upsert_regulated_designation(name).await?),
        None => None,
    };

    let designation_of_origin_id = match &product.detailed_info.designation_of_origin {
        Some(name) => Some(db.upsert_designation_of_origin(name).await?),
        None => None,
    };

    let classification_id = match &product.detailed_info.classification {
        Some(name) => Some(db.upsert_classification(name).await?),
        None => None,
    };

    let ld_product = product.get_ld_product()?;

    let size = product.detailed_info.size.as_ref();

    let sugar = product.detailed_info.sugar_content.as_ref();

    let new_product = ProductUpsertFields {
        saq_code: &product.detailed_info.saq_code,
        upc_code: product.detailed_info.upc_code.as_deref(),
        name: &ld_product.name,
        description: &ld_product.description,
        image_url: &ld_product.image,
        availability: ld_product.offers.availability.db_serialize(),
        item_condition: ld_product.offers.item_condition.db_serialize(),
        price_cad: &ld_product.offers.price,
        abv_percentage: product.detailed_info.abv_percentage,
        container_count: size.as_ref().map(|s| s.container_count),
        container_milliliters: size.as_ref().map(|s| s.container_milliliters),
        product_of_quebec: product
            .detailed_info
            .product_of_quebec
            .as_ref()
            .map(|p| p.db_serialize()),
        sugar_content_equality: sugar.as_ref().map(|s| s.equality.db_serialize()),
        sugar_content_grams_per_liter: sugar.as_ref().map(|s| s.grams_per_liter),
        producer_id,
        promoting_agent_id,
        color_id,
        region_id,
        country_id,
        regulated_designation_id,
        designation_of_origin_id,
        classification_id,
    };

    let product_id = db.upsert_product(new_product).await?;

    let mut special_feature_ids = vec![];
    for special_feature in product.detailed_info.special_features.iter().flatten() {
        let special_feature_id = db.upsert_special_feature(special_feature).await?;
        special_feature_ids.push(special_feature_id);
    }

    db.ensure_product_special_features(product_id, special_feature_ids)
        .await?;

    let mut grape_variety_ids_and_percentages = vec![];
    for variety in product.detailed_info.grape_varieties.iter().flatten() {
        let variety_id = db.upsert_grape_variety(&variety.name).await?;
        grape_variety_ids_and_percentages.push((variety_id, variety.percentage));
    }

    db.ensure_product_grape_varieties(product_id, grape_variety_ids_and_percentages)
        .await?;

    let mut category_ids = vec![];
    for category in product.extract_categories()? {
        let parent_category_id = category_ids.last();
        let category_id = db
            .upsert_category(&category.name, &category.url, parent_category_id.cloned())
            .await?;
        category_ids.push(category_id);
    }

    db.ensure_product_categories(product_id, category_ids)
        .await?;

    Ok(())
}
