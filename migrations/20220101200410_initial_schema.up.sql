create table producers (
  id integer primary key,
  name text not null
) strict;

create unique index producers__name on producers(name);

create table promoting_agents (
  id integer primary key,
  name text not null
);

create unique index promoting_agents__name on promoting_agents(name);

create table colors (
  id integer primary key,
  name text not null
) strict;

create unique index colors__name on colors(name);

create table regions (
  id integer primary key,
  name text not null
) strict;

create unique index regions__name on regions(name);

create table countries (
  id integer primary key,
  name text not null
) strict;

create unique index countries__name on countries(name);

create table grape_varieties (
  id integer primary key,
  name text not null
) strict;

create unique index grape_varieties__name on grape_varieties(name);

create table regulated_designations (
  id integer primary key,
  name text not null
) strict;

create unique index regulated_designations__name on regulated_designations(name);

create table designations_of_origin (
  id integer primary key,
  name text not null
) strict;

create unique index designations_of_origin__name on designations_of_origin(name);

create table classifications (
  id integer primary key,
  name text not null
) strict;

create unique index classifications__name on classifications(name);

create table special_features (
  id integer primary key,
  name text not null
) strict;

create unique index special_features__name on special_features(name);

create table products (
  id integer primary key,
  saq_code text not null,
  upc_code text,
  name text not null,
  description text not null,
  image_url text not null,
  availability text check (availability in ('back_order', 'discontinued', 'in_stock', 'in_store_only', 'limited_availability', 'online_only', 'out_of_stock', 'pre_order', 'pre_sale', 'sold_out')) not null,
  item_condition text check (item_condition in ('damaged', 'new', 'refurbished', 'used')) not null,
  price_cad real check (price_cad > 0) not null,
  producer_id integer references producers(id),
  promoting_agent_id integer references promoting_agents(id),
  abv_percentage real,
  container_count integer,
  container_milliliters integer,
  color_id integer references colors(id),
  region_id integer references regions(id),
  country_id integer references countries(id),
  product_of_quebec text check (product_of_quebec in ('bottled_in_quebec', 'made_in_quebec', 'origine_quebec')),
  sugar_content_equality text check (sugar_content_equality in ('>', '<', '=')),
  sugar_content_grams_per_liter real,
  regulated_designation_id integer references regulated_designations(id),
  designation_of_origin_id integer references designations_of_origin(id),
  classification_id integer references classifications(id),
  created_at text not null default (datetime('now', 'utc')), 
  updated_at text not null default (datetime('now', 'utc'))
) strict;

create unique index products__saq_code on products(saq_code);
create unique index products__upc_code on products(upc_code);

create table product_grape_varieties (
  id integer primary key,
  product_id integer references products(id) not null,
  grape_variety_id integer references grape_varieties(id) not null,
  percentage integer check (percentage between 0 and 100),
  created_at text not null default (datetime('now', 'utc')), 
  updated_at text not null default (datetime('now', 'utc'))
) strict;

create unique index product_grape_varities__product_id__grape_variety_id on product_grape_varieties(product_id, grape_variety_id);

create table product_special_features (
  id integer primary key,
  product_id integer references products(id) not null,
  special_feature_id integer references special_features(id) not null,
  created_at text not null default (datetime('now', 'utc')), 
  updated_at text not null default (datetime('now', 'utc'))
) strict;

create unique index product_special_features__product_id__special_feature_id on product_special_features(product_id, special_feature_id);

create table categories (
  id integer primary key,
  url text not null,
  parent_category_id integer references categories(id),
  name text not null
) strict;

create unique index categories__name on categories(name);

create table product_categories (
  id integer primary key,
  product_id integer references products(id) not null,
  category_id integer references categories(id) not null,
  created_at text not null default (datetime('now', 'utc')), 
  updated_at text not null default (datetime('now', 'utc'))
) strict;

create unique index product_categories__product_id__category_id on product_categories(product_id, category_id);
