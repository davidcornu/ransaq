//! Database querying and persistence.
//!
//! `ransaq` stores crawled data in SQLite using [`sqlx`](sqlx).
//!
//! ## Setup
//!
//! To set up a development database you will need to install
//! [`sqlx-cli`](https://crates.io/crates/sqlx-cli),
//!
//! ```shell
//! cargo install sqlx-cli \
//!     --no-default-features \
//!     --features sqlite \
//!     --features rustls
//! ```
//!
//! create a `.env` file with a `DATABASE_URL`,
//!
//! ```
//! DATABASE_URL=sqlite:ransaq.sqlite
//! ```
//!
//! and finally run
//!
//! ```shell
//! sqlx database setup
//! ```
//!
//! ## Schema
//!
//! You can view the schema by looking through the `/migrations`
//! directory, or by using the `sqlite`[^version] command as follows:
//!
//! ```shell
//! sqlite3 -readonly ransaq.sqlite ".schema"
//! ```
//!
//! [^version]: You will need to be running SQLite version `3.37.0` or later
//! due to the use of `STRICT` tables (<https://www.sqlite.org/releaselog/3_37_0.html>)

mod glue;
pub use glue::DbSerialize;

use color_eyre::eyre::{eyre, Report, Result};
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous,
};
use sqlx::Connection;
use std::str::FromStr;
use std::time::Duration;

/// Returns the SQLite configuration used by `ransaq`.
///
/// It is largely based on Ben Johnson's recommendations on
/// [the Litestream website](https://litestream.io/tips/) and
/// in [this GopherCon Talk](https://www.youtube.com/watch?v=XcAYkriuQ1o).
///
/// The `url` parameter supports the following formats:
/// - `sqlite::memory:`
/// - `sqlite:/path/to/file`
fn sqlite_configuration(url: &str) -> Result<SqliteConnectOptions> {
    let options = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        // https://sqlite.org/pragma.html#pragma_busy_timeout
        .busy_timeout(Duration::from_millis(5000))
        // https://sqlite.org/pragma.html#pragma_defer_foreign_keys
        .foreign_keys(true)
        // https://sqlite.org/pragma.html#pragma_encoding
        .pragma("encoding", "'UTF-8'")
        // https://sqlite.org/pragma.html#pragma_journal_mode
        .journal_mode(SqliteJournalMode::Wal)
        // https://sqlite.org/pragma.html#pragma_synchronous
        .synchronous(SqliteSynchronous::Normal);

    Ok(options)
}

/// Wraps all database logic.
///
/// Note - `Client` is both `Sync` and cheap to `Clone` thanks to
/// [`SqlitePool`](sqlx::sqlite::SqlitePool) being wrapped in an `Arc`.
#[derive(Clone)]
pub struct Client {
    /// The connection pool to use.
    pool: SqlitePool,
}

impl Client {
    /// Returns a new `Client` using the given `url` string.
    ///
    /// See [`sqlite_configuration`] for accepted `url` formats.
    pub async fn new(url: &str) -> Result<Client> {
        let options = sqlite_configuration(url)?;
        let pool = SqlitePoolOptions::new()
            .max_connections(10)
            .connect_with(options)
            .await?;

        Ok(Client { pool })
    }

    /// Returns a new `Client` using the `DATABASE_URL` environment variable.
    ///
    /// See [`sqlite_configuration`] for accepted formats.
    pub async fn new_from_env() -> Result<Client> {
        let url = std::env::var("DATABASE_URL")
            .map_err(|_| eyre!("could not find DATABASE_URL environment variable"))?;

        Ok(Client::new(&url).await?)
    }

    #[cfg(test)]
    async fn migrate(&self) -> Result<()> {
        use sqlx::migrate::Migrator;
        use std::path::Path;

        let migrator = Migrator::new(Path::new("./migrations")).await?;
        migrator.run(&self.pool).await?;

        Ok(())
    }

    /// Uses upserts to make sure there are rows in `product_special_features` for
    /// each of the provided `special_feature_ids`.
    ///
    /// `updated` at is always updated.
    ///
    /// Any entries in `product_special_features` for the given `product_id` that don't
    /// reference any of the provided `special_feature_ids` are subsequently deleted.
    pub async fn ensure_product_special_features(
        &self,
        product_id: i64,
        special_feature_ids: Vec<i64>,
    ) -> Result<()> {
        let mut conn = self.pool.acquire().await?;
        let mut transaction = conn.begin().await?;

        for special_feature_id in &special_feature_ids {
            let ins_result = sqlx::query!(
                r#"insert into product_special_features (product_id, special_feature_id) 
                values (?1, ?2) on conflict do update set updated_at=(datetime('now', 'utc'))"#,
                product_id,
                special_feature_id
            )
            .execute(&mut transaction)
            .await;

            if let Err(err) = ins_result {
                transaction.rollback().await?;
                return Err(Report::from(err));
            }
        }

        let special_feature_id_list = to_value_list(special_feature_ids);

        let del_result = sqlx::query!(
            r#"delete from product_special_features where product_id = ?1 and special_feature_id not in (?2)"#,
            product_id,
            special_feature_id_list
        )
        .execute(&mut transaction)
        .await;

        if let Err(err) = del_result {
            transaction.rollback().await?;
            return Err(Report::from(err));
        }

        transaction.commit().await?;

        Ok(())
    }

    /// Uses upserts to make sure there are rows in `product_grape_varieties`
    /// for each of the provided `(grape_variety_id, percentage)` pairs.
    ///
    /// `updated_at` and `percentage` are always updated to the provided values.
    ///
    /// Any entries in `product_grape_varieties` for the given `product_id` that
    /// don't reference the `grape_variety_id`s in the provided pairs are
    /// subsequently deleted.
    pub async fn ensure_product_grape_varieties(
        &self,
        product_id: i64,
        grape_variety_ids_and_percentages: Vec<(i64, Option<u8>)>,
    ) -> Result<()> {
        let mut conn = self.pool.acquire().await?;
        let mut transaction = conn.begin().await?;

        let mut variety_ids = Vec::with_capacity(grape_variety_ids_and_percentages.len());

        for (grape_variety_id, percentage) in grape_variety_ids_and_percentages {
            let ins_result = sqlx::query!(
                r#"insert into product_grape_varieties (product_id, grape_variety_id, percentage)
                values (?1, ?2, ?3) on conflict do update set
                updated_at=(datetime('now', 'utc')), percentage=excluded.percentage"#,
                product_id,
                grape_variety_id,
                percentage
            )
            .execute(&mut transaction)
            .await;

            if let Err(err) = ins_result {
                transaction.rollback().await?;
                return Err(Report::from(err));
            }

            variety_ids.push(grape_variety_id);
        }

        let variety_id_list = to_value_list(variety_ids);

        let del_result = sqlx::query!(
            r#"delete from product_grape_varieties where product_id = ?1 and grape_variety_id not in (?2)"#,
            product_id,
            variety_id_list
        )
        .execute(&mut transaction)
        .await;

        if let Err(err) = del_result {
            transaction.rollback().await?;
            return Err(Report::from(err));
        }

        transaction.commit().await?;

        Ok(())
    }

    /// Use an upsert query to make sure a row exists in the `categories` table
    /// with the provided `name`, and updating the `url` and `parent_id` fields.
    ///
    /// Returns the row's `id`.
    pub async fn upsert_category(
        &self,
        name: &str,
        url: &str,
        parent_id: Option<i64>,
    ) -> Result<i64> {
        let mut conn = self.pool.acquire().await?;

        let upsert_id = sqlx::query_scalar!(
            r#"insert into categories (name, url, parent_category_id) values (?1, ?2, ?3)
            on conflict do update set url=excluded.url, parent_category_id=excluded.parent_category_id 
            where (url != excluded.url or parent_category_id != excluded.parent_category_id)
            returning id as "id!""#,
            name,
            url,
            parent_id
        ).fetch_optional(&mut conn).await?;

        if let Some(id) = upsert_id {
            return Ok(id);
        }

        Ok(sqlx::query_scalar!(
            r#"select id as "id!" from categories where name = ?1 limit 1"#,
            name
        )
        .fetch_one(&mut conn)
        .await?)
    }

    /// Uses upserts to make sure there are rows in `product_categories` for each of
    /// the provided `category_ids`.
    ///
    /// `updated_at` is always updated.
    ///
    /// Any entries in `product_categories` for the given `product_id` that don't
    /// reference any of the provided `category_ids` are subsequently deleted.
    pub async fn ensure_product_categories(
        &self,
        product_id: i64,
        category_ids: Vec<i64>,
    ) -> Result<()> {
        let mut conn = self.pool.acquire().await?;
        let mut transaction = conn.begin().await?;

        for category_id in &category_ids {
            let ins_result = sqlx::query!(
                r#"insert into product_categories (product_id, category_id)
                values (?1, ?2) on conflict do update set updated_at=(datetime('now', 'utc'))"#,
                product_id,
                category_id
            )
            .execute(&mut transaction)
            .await;

            if let Err(err) = ins_result {
                transaction.rollback().await?;
                return Err(Report::from(err));
            }
        }

        let category_id_list = to_value_list(category_ids);

        let del_result = sqlx::query!(
            r#"delete from product_categories where product_id = ?1 and category_id not in (?2)"#,
            product_id,
            category_id_list
        )
        .execute(&mut transaction)
        .await;

        if let Err(err) = del_result {
            transaction.rollback().await?;
            return Err(Report::from(err));
        }

        transaction.commit().await?;

        Ok(())
    }
}

/// Encodes a list of IDs as a comma-separated string.
///
/// This is used as a workaround[^1] for queries like `where id in (?)` as sqlx doesn't
/// currently support list parameters although there is currently a proposal:
/// <https://github.com/launchbadge/sqlx/issues/875>.
///
/// [^1]: <https://github.com/launchbadge/sqlx/issues/656#issuecomment-689326492>
fn to_value_list(list: impl IntoIterator<Item = i64>) -> String {
    list.into_iter()
        .map(|item| item.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// Contains the necessary parameters to insert a row into
/// the `products` table.
///
/// Callers are responsible for using the necessary `upsert_*` and
/// `ensure_*` methods on [`Client`] to obtain the necessary
/// database `id`s to populate this struct. Note that these are
/// all subject to `FOREIGN KEY` constraints.
///
/// More detail on the shape of this data can be found by reading
/// through the [`saq`](`crate::saq`) docs.
///
/// This struct's fields are kept in alphabetial order to make
/// reasoning about which fields are included in queries easier.
pub struct ProductUpsertFields<'a> {
    /// The alcohol by volume percentage as a float.
    pub abv_percentage: Option<f32>,
    /// The string representation of the [`ItemAvailability`](crate::saq::linked_data::ItemAvailability) enum.
    pub availability: &'a str,
    /// A database `id` from the `classifications` table.
    pub classification_id: Option<i64>,
    /// A database `id` from the `colors` table.
    pub color_id: Option<i64>,
    /// The number of containers for the given product (i.e. 6 cans).
    pub container_count: Option<u8>,
    /// The number of milliliters contained in each container (i.e. 750ml).
    pub container_milliliters: Option<u32>,
    /// A database `id` from the `countries` table.
    pub country_id: Option<i64>,
    /// The product's description.
    pub description: &'a str,
    /// A database `id` from the `designations_of_origin` table.
    pub designation_of_origin_id: Option<i64>,
    /// A URL for an image of the product.
    pub image_url: &'a str,
    /// A string representation of the [`OfferItemCondition`](crate::saq::linked_data::OfferItemCondition) enum.
    pub item_condition: &'a str,
    /// The product's name.
    pub name: &'a str,
    /// The product's price in Canadian Dollars as a float.
    pub price_cad: &'a f64,
    /// A database `id` from the `producers` table.
    pub producer_id: Option<i64>,
    /// A string representation of the [`ProductOfQuebec`](crate::saq::detailed_info::ProductOfQuebec) enum.
    pub product_of_quebec: Option<&'a str>,
    /// A database `id` from the `promoting_agents` table.
    pub promoting_agent_id: Option<i64>,
    /// A database `id` from the `regions` table.
    pub region_id: Option<i64>,
    /// A database `id` from the `regulated_designations` table.
    pub regulated_designation_id: Option<i64>,
    /// The SAQ's unique product identifier.
    pub saq_code: &'a str,
    /// The string representation of the [`SugarContentEquality`](`crate::saq::detailed_info::SugarContentEquality`) enum.
    pub sugar_content_equality: Option<&'a str>,
    /// The number of grams of sugar per liter as a float.
    pub sugar_content_grams_per_liter: Option<f32>,
    /// The product's UPC code.
    pub upc_code: Option<&'a str>,
}

impl Client {
    /// Use an upsert query to ensure a row with the given [`saq_code`](ProductUpsertFields::saq_code)
    /// exists in the `products` table, and update the remaining fields.
    ///
    /// If a row already exists, `updated_at` will be set to the current time and consequently
    /// differ from `created_at`.
    pub async fn upsert_product(&self, fields: ProductUpsertFields<'_>) -> Result<i64> {
        let mut conn = self.pool.acquire().await?;

        // Unfortunately sqlx doesn't support named parameters yet
        // https://github.com/launchbadge/sqlx/issues/199
        let id = sqlx::query_scalar!(
            r#"insert into 
            products (
                abv_percentage,
                availability, 
                classification_id,
                color_id, 
                container_count, 
                container_milliliters,
                country_id, 
                description, 
                designation_of_origin_id,
                image_url,
                item_condition, 
                name, 
                price_cad, 
                producer_id, 
                product_of_quebec,
                promoting_agent_id, 
                region_id,
                regulated_designation_id, 
                saq_code, 
                sugar_content_equality, 
                sugar_content_grams_per_liter,
                upc_code
            )
            values (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22
            )
            on conflict do update set
                updated_at=(datetime('now', 'utc')),
                abv_percentage=excluded.abv_percentage,
                availability=excluded.availability, 
                classification_id=excluded.classification_id,
                color_id=excluded.color_id, 
                container_count=excluded.container_count, 
                container_milliliters=excluded.container_milliliters,
                country_id=excluded.country_id, 
                description=excluded.description, 
                designation_of_origin_id=excluded.designation_of_origin_id,
                image_url=excluded.image_url,
                item_condition=excluded.item_condition, 
                name=excluded.name, 
                price_cad=excluded.price_cad, 
                producer_id=excluded.producer_id, 
                product_of_quebec=excluded.product_of_quebec,
                promoting_agent_id=excluded.promoting_agent_id, 
                region_id=excluded.region_id,
                regulated_designation_id=excluded.regulated_designation_id, 
                -- saq_code omitted
                sugar_content_equality=excluded.sugar_content_equality, 
                sugar_content_grams_per_liter=excluded.sugar_content_grams_per_liter,
                upc_code=excluded.upc_code
            returning id as "id!""#,
            fields.abv_percentage,
            fields.availability,
            fields.classification_id,
            fields.color_id,
            fields.container_count,
            fields.container_milliliters,
            fields.country_id,
            fields.description,
            fields.designation_of_origin_id,
            fields.image_url,
            fields.item_condition,
            fields.name,
            fields.price_cad,
            fields.producer_id,
            fields.product_of_quebec,
            fields.promoting_agent_id,
            fields.region_id,
            fields.regulated_designation_id,
            fields.saq_code,
            fields.sugar_content_equality,
            fields.sugar_content_grams_per_liter,
            fields.upc_code
        )
        .fetch_one(&mut conn)
        .await?;

        Ok(id)
    }
}

/// Generates a method on [`Client`] named using the provided identifier
/// which runs an upsert on the provided table name to make sure a row
/// exists with the given `name`, returning the row's `id`.
macro_rules! generate_upserts_by_name {
    ($($fn:ident => $table:literal),*) => {
        impl Client {
            $(
                #[doc = concat!(
                    "Use an upsert to make sure there is a row in the `",
                    $table,
                    "` table with the given `name`.\n\nReturns the row's `id`.\n\n",
                    "<small>Generated by the [`generate_upserts_by_name`] macro.</small>"
                )]
                pub async fn $fn(&self, name: &str) -> Result<i64> {
                    let mut conn = self.pool.acquire().await?;
                    // Ideally this could use the macro equivalent to benefit from compile-time
                    // checks, however due to how the macro system currently works you can only
                    // pass in a string literal and not a macro expansion.
                    // https://github.com/launchbadge/sqlx/issues/712
                    let upsert_id = sqlx::query_scalar(concat!(
                        "insert into ",
                        $table,
                        " (name) values (?1) on conflict do nothing returning id"
                    ))
                    .bind(name)
                    .fetch_optional(&mut conn)
                    .await?;

                    if let Some(id) = upsert_id {
                        return Ok(id);
                    }

                    Ok(sqlx::query_scalar(concat!(
                        "select id as \"id!\" from ",
                        $table,
                        " where name = ?1 limit 1"
                    ))
                    .bind(name)
                    .fetch_one(&mut conn)
                    .await?)
                }
            )*
        }
    };
}

generate_upserts_by_name!(
    upsert_producer => "producers",
    upsert_promoting_agent => "promoting_agents",
    upsert_color => "colors",
    upsert_region => "regions",
    upsert_country => "countries",
    upsert_grape_variety => "grape_varieties",
    upsert_regulated_designation => "regulated_designations",
    upsert_designation_of_origin => "designations_of_origin",
    upsert_classification => "classifications",
    upsert_special_feature => "special_features"
);

#[cfg(test)]
mod tests {
    use super::*;
    use paste::paste;
    use sqlx::migrate::MigrateDatabase;
    use tokio::sync::OnceCell;

    static SHARED_CLIENT: OnceCell<Client> = OnceCell::const_new();

    async fn get_client() -> Result<&'static Client> {
        async fn init_client() -> Result<Client> {
            let url = "sqlite:ransaq.test.sqlite";

            if sqlx::Sqlite::database_exists(url).await? {
                sqlx::Sqlite::drop_database(url).await?;
            }

            let client = Client::new(url).await?;
            client.migrate().await?;

            Ok(client)
        }

        Ok(SHARED_CLIENT.get_or_try_init(init_client).await?)
    }

    macro_rules! test_upserts_by_name {
        ($($fn:ident),*) => {
            $(
                paste! {
                    #[tokio::test]
                    async fn [<test_ $fn>]() -> Result<()> {
                        let client = get_client().await?;

                        let attempt_1 = client.$fn("Upserted Name").await?;
                        let attempt_2 = client.$fn("Upserted Name").await?;

                        assert_eq!(attempt_1, attempt_2);
                        Ok(())
                    }
                }
            )*
        };
    }

    test_upserts_by_name!(
        upsert_producer,
        upsert_promoting_agent,
        upsert_color,
        upsert_region,
        upsert_country,
        upsert_grape_variety,
        upsert_regulated_designation,
        upsert_designation_of_origin,
        upsert_classification,
        upsert_special_feature
    );
}
