//! Parsing and cleanup logic to extract data out of the Detailed Info
//! section of product pages.

use color_eyre::eyre::{eyre, Result, WrapErr};
use lazy_static::lazy_static;
use regex::Regex;
use std::collections::HashMap;
use std::str::FromStr;

/// Data extracted from the Detailed Info section of product pages.
#[derive(Debug)]
pub struct DetailedInfo {
    /// The product's producer (i.e. "The Absolut Company")
    pub producer: Option<String>,
    /// The SAQ's unique identifier for the product
    pub saq_code: String,
    /// The product's promoting agent/importer (i.e. "La QV Inc. (GB)")
    pub promoting_agent: Option<String>,
    /// The product's percentage of alcohol (i.e. "12.3 %"")
    pub abv_percentage: Option<f32>,
    /// The number of containers and volume contained in each one. If
    /// no number is provided it is assumed to be 1.
    ///
    /// Examples: "1 L", "750 mL", "750 ml", "2.25 L", "6 x 296 ml"
    pub size: Option<Size>,
    /// The product's color (i.e. "Amber", "Beige", "Black")
    ///
    /// This is used reguardless of the product's type so some may make
    /// more sense for wine or beer.
    pub color: Option<String>,
    /// The region where the product was produced (i.e. "Jura")
    pub region: Option<String>,
    /// The product's UPC code
    pub upc_code: Option<String>,
    /// The country where the product was produced (i.e. "Argentina")
    pub country: Option<String>,
    /// The specific "Product of Québec" labelling
    ///
    /// This is one of "Bottled in Québec", "Made in Québec", or "Origine Québec"
    pub product_of_quebec: Option<ProductOfQuebec>,
    /// Comma-separated grape varieties, optionally including a percentage
    ///
    /// Sometimes the percentages don't add up to 100, or the list includes
    /// grape varieties adding up to 100% followed by others without a
    /// percentage.
    ///
    /// Examples:
    /// - "Zibibbo 100 %"
    /// - "Zinfandel 95 %, Other grape variety (ies) 5 %"
    /// - "Zinfandel 86 %, Petite sirah 7 %, Alicante Bouschet 6 %, Mourvèdre 1 %"
    /// - "Zinfandel 80 %, Petite sirah 16 %, Carignan 4 %, Cabernet sauvignon"
    /// - "Chardonnay"
    pub grape_varieties: Option<Vec<GrapeVariety>>,
    /// The product's sugar content in grams per liter, optionally preceeded by
    /// a `<` or `>` sign to indicate imprecise measurements.
    ///
    /// Examples: "<1.2 g/L", ">60 g/L", "2.9 g/L"
    pub sugar_content: Option<SugarContent>,
    /// Any regulated designations the product is subject to (i.e. "Appellation origine controlée (AOC)")
    pub regulated_designation: Option<String>,
    /// The product's designated origin (i.e. "Arbois", "Bourgogne Hautes-Côtes de Beaune")
    pub designation_of_origin: Option<String>,
    /// The product's classification (i.e. "1er cru classé", "Gran reserva")
    pub classification: Option<String>,
    /// A comma-separated list of special features (i.e. whether the product is organic).
    ///
    /// Examples:
    /// - "Natural Wine, Orange Wine, Organic product"
    /// - "A low alcohol (0,6 to 9,5%), Kosher (Mevushal)"
    /// - "Orange Wine"
    pub special_features: Option<Vec<String>>,
}

impl DetailedInfo {
    /// Converts a `HashMap` of keys and values extracted from a product page's HTML
    /// via [`extract_detailed_info`](super::extract_detailed_info) to a
    /// [`DetailedInfo`] struct, performing all the necessary parsing to provide
    /// more expressive data types.
    pub fn from_hash_map(mut map: HashMap<String, String>) -> Result<Self> {
        let abv_percentage = match map.remove("Degree of alcohol") {
            Some(text) => Some(parse_abv(&text)?),
            None => None,
        };

        let size = match map.remove("Size") {
            Some(text) => Some(parse_size(&text)?),
            None => None,
        };

        let product_of_quebec = match map.remove("Product of Québec") {
            Some(text) => Some(parse_product_of_quebec(&text)?),
            None => None,
        };

        let grape_varieties = match map.remove("Grape variety") {
            Some(text) => Some(parse_grape_varieties(&text)?),
            None => None,
        };

        let sugar_content = match map.remove("Sugar content") {
            Some(text) => Some(parse_sugar_content(&text)?),
            None => None,
        };

        let special_features = map.remove("Special feature").map(|text| {
            text.split(", ")
                .map(|part| part.to_string())
                .collect::<Vec<_>>()
        });

        Ok(DetailedInfo {
            producer: map.remove("Producer"),
            saq_code: map
                .remove("SAQ code")
                .ok_or_else(|| eyre!("SAQ code not found"))?,
            promoting_agent: map.remove("Promoting agent"),
            abv_percentage,
            size,
            color: map.remove("Color"),
            region: map.remove("Region"),
            upc_code: map.remove("UPC code"),
            country: map.remove("Country"),
            product_of_quebec,
            grape_varieties,
            sugar_content,
            regulated_designation: map.remove("Regulated Designation"),
            designation_of_origin: map.remove("Designation of origin"),
            classification: map.remove("Classification"),
            special_features,
        })
    }
}

lazy_static! {
    #[doc(hidden)]
    static ref ABV_RE: Regex = Regex::new(r"\A(\d+(\.\d+)?) %\z").unwrap();
}

/// Converts a string indicating the alcohol by volume percentage into a float.
fn parse_abv(text: &str) -> Result<f32> {
    let num = ABV_RE
        .captures(text)
        .and_then(|c| c.get(1))
        .ok_or_else(|| eyre!("failed to match {:?}", text))?
        .as_str();

    f32::from_str(num).wrap_err_with(|| format!("failed to parse {num:?} as float"))
}

/// The product's size
#[derive(Debug)]
pub struct Size {
    /// The number of containers for the given product
    pub container_count: u8,
    /// The volume in milliliters in each container
    pub container_milliliters: u32,
}

lazy_static! {
    #[doc(hidden)]
    static ref SIZE_RE: Regex = Regex::new(r"\A((\d+) x )?(\d+(\.\d+)?) (mL|ml|L)\z").unwrap();
}

/// Converts the product's size string (i.e. "6 x 200ml") into a [`Size`]
fn parse_size(text: &str) -> Result<Size> {
    let captures = SIZE_RE
        .captures(text)
        .ok_or_else(|| eyre!("failed to match {:?}", text))?;

    let container_count = match captures.get(2) {
        Some(m) => {
            let num = m.as_str();
            u8::from_str(num).wrap_err_with(|| format!("failed to parse {num:?} as u8"))?
        }
        None => 1,
    };

    let num_text = captures.get(3).expect("non-optional capture").as_str();
    let num =
        f32::from_str(num_text).wrap_err_with(|| format!("failed to parse {num_text:?} as f32"))?;
    let unit = captures.get(5).expect("non-optional capture").as_str();

    let container_milliliters = match unit {
        "mL" | "ml" => num.ceil() as u32,
        "L" => (num * 1000.0).ceil() as u32,
        _ => unreachable!("not permitted by regex"),
    };

    Ok(Size {
        container_count,
        container_milliliters,
    })
}

/// The product's sugar content.
#[derive(Debug)]
pub struct SugarContent {
    /// The quantity of sugar expressed as grams per liter
    pub grams_per_liter: f32,
    /// Whether the actual sugar quantity is less than, greater than, or equal to `grams_per_liter`.
    pub equality: SugarContentEquality,
}

/// Whether the actual sugar quantity is less than, greater than, or equal to `grams_per_liter`.
#[derive(Debug, PartialEq)]
pub enum SugarContentEquality {
    /// The actual sugar content is **greater than** what's indicated.
    GreaterThan,
    /// The actual sugar content is **less than** what's indicated.
    LessThan,
    /// The actual sugar content is **equal** to what's indicated.
    Equal,
}

lazy_static! {
    #[doc(hidden)]
    static ref SUGAR_CONTENT_RE: Regex = Regex::new(r"\A(<|>)?(\d+(\.\d+)?) g/L\z").unwrap();
}

/// Converts the string representation of the product's sugar content into a [`SugarContent`].
fn parse_sugar_content(text: &str) -> Result<SugarContent> {
    let captures = SUGAR_CONTENT_RE
        .captures(text)
        .ok_or_else(|| eyre!("failed to match {:?}", text))?;

    let equality = match captures.get(1).map(|m| m.as_str()) {
        Some(">") => SugarContentEquality::GreaterThan,
        Some("<") => SugarContentEquality::LessThan,
        None => SugarContentEquality::Equal,
        _ => unreachable!("not permitted by regex"),
    };

    let num_text = captures.get(2).expect("non-optional capture").as_str();
    let grams_per_liter =
        f32::from_str(num_text).wrap_err_with(|| format!("failed to parse {num_text:?} as f32"))?;

    Ok(SugarContent {
        equality,
        grams_per_liter,
    })
}

/// The grape variety and percentage of it present in the product
#[derive(Debug)]
pub struct GrapeVariety {
    /// The grape variety name (i.e. chardonnay).
    pub name: String,
    /// The percentage of the grape variety present in the product.
    pub percentage: Option<u8>,
}

lazy_static! {
    #[doc(hidden)]
    static ref GRAPE_VARIETY_PERCENT_RE: Regex = Regex::new(r"(\s(\d+)\s%)\z").unwrap();
}

/// Converts the string representaiton of the grape varieties present in the product to
/// a `Vec` of [`GrapeVariety`].
fn parse_grape_varieties(text: &str) -> Result<Vec<GrapeVariety>> {
    let mut varieties = vec![];

    for part in text.split(',') {
        let part = part.trim();

        let mut name = part;
        let mut percentage = None;

        let percentage_match = GRAPE_VARIETY_PERCENT_RE.captures(part);

        if let Some(captures) = percentage_match {
            let offset = captures.get(1).expect("non-optional capture").start();
            let percentage_text = captures.get(2).expect("non-optional capture").as_str();
            percentage = Some(u8::from_str(percentage_text).wrap_err_with(|| {
                format!("failed to parse percentage from {part:?} ({percentage_text:?}) as u8",)
            })?);
            name = part[0..offset].trim();
        }

        if name.is_empty() {
            return Err(eyre!("could not detect name in {:?}", part));
        }

        varieties.push(GrapeVariety {
            name: name.to_string(),
            percentage,
        });
    }

    Ok(varieties)
}

/// The specifics of the "Product of Québec" label
///
/// <https://www.lapresse.ca/gourmand/alcools/2020-06-04/les-produits-quebecois-mieux-identifies-par-la-saq>
/// <https://www.laterre.ca/actualites/economie/nouvelle-distinction-entre-les-produits-du-quebec-a-la-saq>
#[derive(Debug, PartialEq)]
pub enum ProductOfQuebec {
    /// The final product was bottled in Québec.
    BottledIn,
    /// The product was made in Québec, partially or fully from ingredients
    /// sourced outside of Québec.
    MadeIn,
    /// The product was made in Québec using ingredients from Québec.
    Origine,
}

/// Converts the string representation of the "Product of Québec" label into the
/// appropriate [`ProductOfQuebec`] enum variant.
fn parse_product_of_quebec(text: &str) -> Result<ProductOfQuebec> {
    match text {
        "Bottled in Québec" => Ok(ProductOfQuebec::BottledIn),
        "Made in Québec" => Ok(ProductOfQuebec::MadeIn),
        "Origine Québec" => Ok(ProductOfQuebec::Origine),
        _ => Err(eyre!("{:?} is not a valid value", text)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_abv() {
        let valid = parse_abv("12.5 %").unwrap();
        assert_eq!(12.5, valid);

        let valid_no_decimal = parse_abv("12 %").unwrap();
        assert_eq!(12.0, valid_no_decimal);

        let wrong_format_err = parse_abv(" 12 ").unwrap_err();
        assert_eq!("failed to match \" 12 \"", wrong_format_err.to_string());
    }

    #[test]
    fn text_parse_size() {
        let one_liter = parse_size("1 L").unwrap();
        assert_eq!(1, one_liter.container_count);
        assert_eq!(1000, one_liter.container_milliliters);

        let four_times_300_ml = parse_size("4 x 300 ml").unwrap();
        assert_eq!(4, four_times_300_ml.container_count);
        assert_eq!(300, four_times_300_ml.container_milliliters);

        let decimal_liter = parse_size("2.25 L").unwrap();
        assert_eq!(1, decimal_liter.container_count);
        assert_eq!(2250, decimal_liter.container_milliliters);

        let big_l = parse_size("750 mL").unwrap();
        assert_eq!(1, big_l.container_count);
        assert_eq!(750, big_l.container_milliliters);
    }

    #[test]
    fn test_parse_grape_varieties() {
        let one = parse_grape_varieties("Nero d'Avola\u{a0}100\u{a0}%").unwrap();
        assert_eq!(1, one.len());
        assert_eq!("Nero d'Avola", one[0].name);
        assert_eq!(Some(100), one[0].percentage);

        let two = parse_grape_varieties("Zinfandel 95 %, Other grape variety (ies) 5 %").unwrap();
        assert_eq!(2, two.len());
        assert_eq!("Zinfandel", two[0].name);
        assert_eq!(Some(95), two[0].percentage);
        assert_eq!("Other grape variety (ies)", two[1].name);
        assert_eq!(Some(5), two[1].percentage);

        let three = parse_grape_varieties(
            "Zinfandel 80 %, Petite sirah 16 %, Mourvèdre 4 %, Cabernet sauvignon",
        )
        .unwrap();
        assert_eq!(4, three.len());
        assert_eq!("Zinfandel", three[0].name);
        assert_eq!(Some(80), three[0].percentage);
        assert_eq!("Petite sirah", three[1].name);
        assert_eq!(Some(16), three[1].percentage);
        assert_eq!("Mourvèdre", three[2].name);
        assert_eq!(Some(4), three[2].percentage);
        assert_eq!("Cabernet sauvignon", three[3].name);
        assert_eq!(None, three[3].percentage);

        let four = parse_grape_varieties("Chardonnay").unwrap();
        assert_eq!(1, four.len());
        assert_eq!("Chardonnay", four[0].name);
        assert_eq!(None, four[0].percentage);

        let five = parse_grape_varieties("Sainte-Croix\u{a0}5\u{a0}%").unwrap();
        assert_eq!(1, five.len());
        assert_eq!("Sainte-Croix", five[0].name);
        assert_eq!(Some(5), five[0].percentage);

        let six = parse_grape_varieties("Muscat de N.Y.\u{a0}25\u{a0}%").unwrap();
        assert_eq!(1, six.len());
        assert_eq!("Muscat de N.Y.", six[0].name);
        assert_eq!(Some(25), six[0].percentage);
    }

    #[test]
    fn test_parse_sugar_content() {
        let one = parse_sugar_content("<1.2 g/L").unwrap();
        assert_eq!(1.2, one.grams_per_liter);
        assert_eq!(SugarContentEquality::LessThan, one.equality);

        let two = parse_sugar_content(">60 g/L").unwrap();
        assert_eq!(60.0, two.grams_per_liter);
        assert_eq!(SugarContentEquality::GreaterThan, two.equality);

        let three = parse_sugar_content("2.9 g/L").unwrap();
        assert_eq!(2.9, three.grams_per_liter);
        assert_eq!(SugarContentEquality::Equal, three.equality);
    }
}
