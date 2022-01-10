//! HTTP logic to interact with the SAQ website

pub mod detailed_info;
pub mod linked_data;

use color_eyre::eyre::{eyre, Result, WrapErr};
use lazy_static::lazy_static;
use linked_data::{Entity, ItemListElement, LinkedData, OfferCatalog, Product, WebPage};
use reqwest::Url;
use scraper::Selector;
use std::{collections::HashMap, time::Instant};
use tracing::{info, info_span};

/// Provides a number of methods to interact with the SAQ website
///
/// Note - `Client` is both `Sync` and cheap to `Clone` thanks to
/// [`reqwest::Client`] being wrapped in an `Arc`.
#[derive(Clone)]
pub struct Client {
    /// The HTTP client to use.
    reqwest_client: reqwest::Client,
}

/// The HTTP User-Agent used for all requests. This was used as an easy default
/// during development so it is not know whether something that better reflects
/// the intended use would cause requests to be blocked or throttled.
const DEFAULT_USER_AGENT: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:94.0) Gecko/20100101 Firefox/94.0";

impl Client {
    /// Builds a `Client`
    pub fn new() -> Result<Client> {
        let reqwest_client = reqwest::Client::builder()
            .user_agent(DEFAULT_USER_AGENT)
            .build()?;

        Ok(Client { reqwest_client })
    }
}

lazy_static! {
    #[doc(hidden)]
    static ref CURRENT_PAGE_SELECTOR: Selector =
        Selector::parse(".pages .pages-items .current .page span:nth-child(2)").unwrap();
}

impl Client {
    /// Fetches a single page of the SAQ product catalog using the default sorting
    /// (by availability), and returns a list of JSON-LD [`Product`] entries.
    ///
    /// Will return `None` if `page_number` has reached past the end.
    ///
    /// The enpoint also provides the following query parameters
    /// - `product_list_limit` (defaults to `24`)
    /// - `product_list_order` (defaults to `availability`)
    /// however including them or deviating from the defaults adds a nontrivial
    /// amount of latency.
    pub async fn page(&self, page_number: u32) -> Result<Option<Vec<Product>>> {
        let url = Url::parse_with_params(
            "https://www.saq.com/en/products",
            &[("p", &page_number.to_string())],
        )?;

        let span = info_span!("page", %url);
        let span_guard = span.enter();

        info!("request");
        let start = Instant::now();

        let res = self
            .reqwest_client
            .get(url)
            .header("accept", "text/html")
            .send()
            .await?;

        info!(status = %res.status(), time = ?start.elapsed() , "response");

        let body = res.text().await?;
        let document = scraper::html::Html::parse_document(&body);

        let current_page = document
            .select(&CURRENT_PAGE_SELECTOR)
            .map(|e| {
                let page_number = e.text().collect::<String>();
                page_number.parse::<u32>().wrap_err_with(|| {
                    format!(
                        "failed to convert page number {:?} to integer",
                        &page_number
                    )
                })
            })
            .next()
            .ok_or_else(|| eyre!("could not find pagination on page"))??;

        // saq.com's pagination wraps around rather than render an empy page
        if current_page != page_number {
            return Ok(None);
        }

        let linked_data = extract_linked_data(&document)?;

        let products = linked_data
            .iter()
            .find_map(|ld| {
                if let LinkedData::WebPage(WebPage {
                    main_entity:
                        Some(Entity::OfferCatalog(OfferCatalog {
                            item_list_element, ..
                        })),
                    ..
                }) = ld
                {
                    Some(
                        item_list_element
                            .iter()
                            .filter_map(|e| {
                                if let ItemListElement::Product(product) = e {
                                    Some(product)
                                } else {
                                    None
                                }
                            })
                            .cloned()
                            .collect::<Vec<_>>(),
                    )
                } else {
                    None
                }
            })
            .unwrap();

        drop(span_guard);

        Ok(Some(products))
    }
}

lazy_static! {
    #[doc(hidden)]
    static ref LD_SCRIPT_SELECTOR: Selector =
        Selector::parse("script[type='application/ld+json']").unwrap();
}

/// Finds the JSON-LD `<script>` tag on the page and parses its contents into
/// [`LinkedData`] entries using [`serde_json`].
fn extract_linked_data(document: &scraper::Html) -> Result<Vec<LinkedData>> {
    Ok(document
        .select(&LD_SCRIPT_SELECTOR)
        .map(|e| serde_json::from_str::<LinkedData>(&e.inner_html()))
        .collect::<Result<Vec<_>, _>>()?)
}

/// Contains all the data extracted from a product page
#[derive(Debug)]
pub struct ExtractedProduct {
    /// Parsed JSON-LD data contained in a `<script>` tag
    pub linked_data: Vec<LinkedData>,
    /// Product metadata from the "Detailed Info" section of the page
    pub detailed_info: detailed_info::DetailedInfo,
}

impl ExtractedProduct {
    /// Convenience method to extract a JSON-LD [`Product`] (if present)
    /// from the other [`LinkedData`] entries.
    pub fn get_ld_product(&self) -> Result<&Product> {
        let ld_product = self
            .linked_data
            .iter()
            .find_map(|ld| match ld {
                LinkedData::Product(product) => Some(product),
                _ => None,
            })
            .ok_or_else(|| eyre!("missing product linked data"))?;

        Ok(ld_product)
    }
}

/// One of the product's categories.
///
/// These typically go from broad ("Wine") to specific ("White wine")
pub struct Category {
    /// The category's name
    pub name: String,
    /// The category listing URL (i.e. <https://www.saq.com/en/products/wine/white-wine>)
    pub url: String,
}

impl ExtractedProduct {
    /// Converts a [`LinkedData::BreadcrumbList`] (if present) to [`Category`] entries.
    pub fn extract_categories(&self) -> Result<Vec<Category>> {
        let ld_breadcrumbs = self
            .linked_data
            .iter()
            .find_map(|ld| match ld {
                LinkedData::BreadcrumbList(breadcrumbs) => Some(breadcrumbs),
                _ => None,
            })
            .ok_or_else(|| eyre!("missing breadcrumb list linked data"))?;

        let categories = ld_breadcrumbs
            .item_list_element
            .iter()
            .filter_map(|li| {
                // Breadcrumbs include the home page, products, and the product itself.
                // We're only interested in product categories.
                if li.item.id.starts_with("https://www.saq.com/en/products/") {
                    Some(Category {
                        name: li.item.name.clone(),
                        url: li.item.id.clone(),
                    })
                } else {
                    None
                }
            })
            .collect();

        Ok(categories)
    }
}

lazy_static! {
    #[doc(hidden)]
    static ref DETAILED_INFO_SELECTOR: Selector =
        scraper::Selector::parse("#product-data-item-additional ul li [data-th]").unwrap();
}

/// Traverses through the "Detailed Info" section of the product page to key-value
/// pairs (i.e. "Designation of origin" -> "Mercurey") which are further processed
/// into a [`DetailedInfo`](detailed_info::DetailedInfo) struct.
fn extract_detailed_info(document: &scraper::Html) -> Result<detailed_info::DetailedInfo> {
    let detailed_info_hash = document
        .select(&DETAILED_INFO_SELECTOR)
        .filter_map(|e| {
            e.value().attr("data-th").map(|key| {
                (
                    key.to_owned(),
                    e.text().collect::<String>().trim().to_owned(),
                )
            })
        })
        .collect::<HashMap<_, _>>();

    detailed_info::DetailedInfo::from_hash_map(detailed_info_hash)
}

impl Client {
    /// Fetch and extract data from a product page
    pub async fn product(&self, product: &Product) -> Result<ExtractedProduct> {
        let product_url = &product.offers.url;

        let span = info_span!("product", %product_url);
        let span_guard = span.enter();

        info!("request");
        let start = Instant::now();

        let res = self
            .reqwest_client
            .get(product_url)
            .header("accept", "text/html")
            .send()
            .await?;

        info!(status = %res.status(), time = ?start.elapsed() , "response");

        let body = res.text().await?;
        let document = scraper::Html::parse_document(&body);

        let linked_data = extract_linked_data(&document)?;
        let detailed_info = extract_detailed_info(&document)?;

        drop(span_guard);

        Ok(ExtractedProduct {
            linked_data,
            detailed_info,
        })
    }
}
