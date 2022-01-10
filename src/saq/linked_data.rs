//! Just enough JSON-LD/Schema.org support to parse what we need

use serde::Deserialize;

/// The subset of [`Thing`](https://schema.org/Thing) included in the SAQ's JSON-LD
#[derive(Deserialize, Debug)]
#[serde(tag = "@type")]
pub enum LinkedData {
    /// <https://schema.org/WebSite>
    WebSite,
    /// A [`BreadcrumbList`]
    BreadcrumbList(BreadcrumbList),
    /// A [`WebPage`]
    WebPage(WebPage),
    /// A [`Product`]
    Product(Product),
    /// Fallback for anything else
    #[serde(other)]
    Other,
}

/// <https://schema.org/BreadcrumbList>
#[derive(Deserialize, Debug)]
pub struct BreadcrumbList {
    /// A partial implementation of [`itemListElement`](https://schema.org/itemListElement)
    /// that assumes only [`ListItem`](https://schema.org/ListItem)s are present.
    #[serde(rename(deserialize = "itemListElement"))]
    pub item_list_element: Vec<ListItem>,
}

/// <https://schema.org/itemListElement>
#[derive(Deserialize, Debug)]
#[serde(tag = "@type")]
pub enum ItemListElement {
    /// A [`ListItem`]
    ListItem(ListItem),
    /// A [`Product`]
    Product(Product),
    /// Fallback for anything else
    #[serde(other)]
    Other,
}

/// <https://schema.org/itemListElement>
#[derive(Deserialize, Debug)]
pub struct ListItem {
    /// The entity represented
    pub item: Item,
    /// <https://schema.org/position>
    pub position: i32,
}

/// <https://schema.org/ListItem>
#[derive(Deserialize, Debug)]
pub struct Item {
    /// <https://www.w3.org/TR/2014/REC-json-ld-20140116/#node-identifiers>
    #[serde(rename(deserialize = "@id"))]
    pub id: String,
    /// <https://schema.org/name>
    pub name: String,
}

/// <https://schema.org/WebPage>
#[derive(Deserialize, Debug)]
pub struct WebPage {
    /// <https://schema.org/mainEntity>
    #[serde(rename(deserialize = "mainEntity"))]
    pub main_entity: Option<Entity>,
    /// <https://schema.org/url>
    pub url: Option<String>,
}

/// Subset of [`Thing`](<https://schema.org/Thing>) included in the SAQ's
/// [`WebPage`]s.
#[derive(Deserialize, Debug)]
#[serde(tag = "@type")]
pub enum Entity {
    /// An [`OfferCatalog`]
    OfferCatalog(OfferCatalog),
    /// Fallback for anything else
    #[serde(other)]
    Other,
}

/// <https://schema.org/OfferCatalog>
#[derive(Deserialize, Debug)]
pub struct OfferCatalog {
    /// <https://schema.org/name>
    pub name: String,
    /// <https://schema.org/url>
    pub url: String,
    /// <https://schema.org/numberOfItems>
    #[serde(rename(deserialize = "numberOfItems"))]
    pub number_of_items: i32,
    /// <https://schema.org/itemListElement>
    #[serde(rename(deserialize = "itemListElement"))]
    pub item_list_element: Vec<ItemListElement>,
}

/// <https://schema.org/Product>
#[derive(Deserialize, Debug, Clone)]
pub struct Product {
    /// <https://schema.org/description>
    pub description: String,
    /// <https://schema.org/image>
    pub image: String,
    /// <https://schema.org/name>
    pub name: String,
    /// <https://schema.org/offers>
    pub offers: Offer,
    /// <https://schema.org/sku>
    ///
    /// For the SAQ this identical to [`saq_code`](super::detailed_info::DetailedInfo::saq_code)
    pub sku: String,
    /// <https://schema.org/category>
    pub category: Option<String>,
}

/// <https://schema.org/Offer>
#[derive(Deserialize, Debug, Clone)]
pub struct Offer {
    /// See [`ItemAvailability`]
    pub availability: ItemAvailability,
    /// See [`OfferItemCondition`]
    #[serde(rename(deserialize = "itemCondition"))]
    pub item_condition: OfferItemCondition,
    /// <https://schema.org/price>
    pub price: f64,
    /// <https://schema.org/priceCurrency>
    #[serde(rename(deserialize = "priceCurrency"))]
    pub price_currency: String,
    /// <https://schema.org/url>
    ///
    /// For the SAQ this is the product page URL.
    pub url: String,
}

#[derive(Deserialize, Debug, Clone)]
/// <https://schema.org/ItemAvailability>
pub enum ItemAvailability {
    /// <http://schema.org/BackOrder>
    #[serde(rename(deserialize = "http://schema.org/BackOrder"))]
    BackOrder,
    /// <http://schema.org/Discontinued>
    #[serde(rename(deserialize = "http://schema.org/Discontinued"))]
    Discontinued,
    /// <http://schema.org/InStock>
    #[serde(rename(deserialize = "http://schema.org/InStock"))]
    InStock,
    /// <http://schema.org/InStoreOnly>
    #[serde(rename(deserialize = "http://schema.org/InStoreOnly"))]
    InStoreOnly,
    /// <http://schema.org/LimitedAvailability>
    #[serde(rename(deserialize = "http://schema.org/LimitedAvailability"))]
    LimitedAvailability,
    /// <http://schema.org/OnlineOnly>
    #[serde(rename(deserialize = "http://schema.org/OnlineOnly"))]
    OnlineOnly,
    /// <http://schema.org/OutOfStock>
    #[serde(rename(deserialize = "http://schema.org/OutOfStock"))]
    OutOfStock,
    /// <http://schema.org/PreOrder>
    #[serde(rename(deserialize = "http://schema.org/PreOrder"))]
    PreOrder,
    /// <http://schema.org/PreSale>
    #[serde(rename(deserialize = "http://schema.org/PreSale"))]
    PreSale,
    /// <http://schema.org/SoldOut>
    #[serde(rename(deserialize = "http://schema.org/SoldOut"))]
    SoldOut,
}

/// <https://schema.org/OfferItemCondition>
#[derive(Deserialize, Debug, Clone)]
pub enum OfferItemCondition {
    /// <https://schema.org/DamagedCondition>
    #[serde(rename(deserialize = "DamagedCondition"))]
    Damaged,
    /// <https://schema.org/NewCondition>
    #[serde(rename(deserialize = "NewCondition"))]
    New,
    /// <https://schema.org/RefurbishedCondition>
    #[serde(rename(deserialize = "RefurbishedCondition"))]
    Refurbished,
    /// <https://schema.org/UsedCondition>
    #[serde(rename(deserialize = "UsedCondition"))]
    Used,
}
