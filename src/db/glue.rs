//! Serialization logic necessary to put [`saq`](crate::saq) types
//! into the database.

use crate::saq::detailed_info::ProductOfQuebec;
use crate::saq::detailed_info::SugarContentEquality;
use crate::saq::linked_data::ItemAvailability;
use crate::saq::linked_data::OfferItemCondition;

/// Utility trait to add database serialization logic to types that
/// shouldn't have to know or care about it.
pub trait DbSerialize {
    /// Given `&self`, return a suitable string representation[^check].
    ///
    /// [^check]: Any values returned here should have matching [`CHECK`
    /// constraints](https://sqlite.org/lang_createtable.html#check_constraints)
    /// in the database schema.
    fn db_serialize(&self) -> &str;
}

impl DbSerialize for ItemAvailability {
    fn db_serialize(&self) -> &str {
        match self {
            ItemAvailability::BackOrder => "back_order",
            ItemAvailability::Discontinued => "discontinued",
            ItemAvailability::InStock => "in_stock",
            ItemAvailability::InStoreOnly => "in_store_only",
            ItemAvailability::LimitedAvailability => "limited_availability",
            ItemAvailability::OnlineOnly => "online_only",
            ItemAvailability::OutOfStock => "out_of_stock",
            ItemAvailability::PreOrder => "pre_order",
            ItemAvailability::PreSale => "pre_sale",
            ItemAvailability::SoldOut => "sold_out",
        }
    }
}

impl DbSerialize for OfferItemCondition {
    fn db_serialize(&self) -> &str {
        match self {
            OfferItemCondition::Damaged => "damaged",
            OfferItemCondition::New => "new",
            OfferItemCondition::Refurbished => "refurbished",
            OfferItemCondition::Used => "used",
        }
    }
}

impl DbSerialize for ProductOfQuebec {
    fn db_serialize(&self) -> &str {
        match self {
            ProductOfQuebec::BottledIn => "bottled_in_quebec",
            ProductOfQuebec::MadeIn => "made_in_quebec",
            ProductOfQuebec::Origine => "origine_quebec",
        }
    }
}

impl DbSerialize for crate::saq::detailed_info::SugarContentEquality {
    fn db_serialize(&self) -> &str {
        match self {
            SugarContentEquality::GreaterThan => ">",
            SugarContentEquality::LessThan => "<",
            SugarContentEquality::Equal => "=",
        }
    }
}
