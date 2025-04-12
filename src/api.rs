use std::collections::HashMap;

use chrono::Datelike;

fn get_graphql_query(city_id: CityId) -> String {
    format!(
        r#"{{ "operationName": "GetCategories", "variables": {{ "currentPage": 1, "filters": {{ "available_to_book": {{ "eq": "179" }}, "category_uid": {{ "eq": "Nw==" }}, "city": {{ "eq": "{}" }} }}, "pageSize": 100, "sort": {{ "available_startdate": "ASC" }} }}, "query": "query GetCategories($pageSize: Int!, $currentPage: Int!, $filters: ProductAttributeFilterInput!, $sort: ProductAttributeSortInput) {{ products( pageSize: $pageSize, currentPage: $currentPage, filter: $filters, sort: $sort ) {{ ...ProductsFragment, __typename }} }} fragment ProductsFragment on Products {{ sort_fields {{ options {{ label, value, __typename }}, __typename }}, aggregations {{ label, count, attribute_code, options {{ label, count, value, __typename }}, position, __typename }}, items {{ name, sku, city, url_key, available_to_book, available_startdate, next_contract_startdate, current_lottery_subscribers, building_name, finishing, living_area, no_of_rooms, resident_type, offer_text_two, offer_text, maximum_number_of_persons, type_of_contract, price_analysis_text, allowance_price, floor, basic_rent, lumpsum_service_charge, inventory, caretaker_costs, cleaning_common_areas, energy_common_areas, energy_label, minimum_stay, allowance_price, price_range {{ minimum_price {{ regular_price {{ value, currency, __typename }}, final_price {{ value, currency, __typename }}, __typename }}, maximum_price {{ regular_price {{ value, currency, __typename }}, final_price {{ value, currency, __typename }}, __typename }}, __typename }} , __typename }}, total_count, __typename }}" }}"#,
        city_id.0
    )
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct CityId(u64);

#[derive(Clone, Copy, PartialEq, Eq, Hash, derive_more::Display, derive_more::FromStr)]
pub enum City {
    Delft,
    Eindhoven,
    DenHaag,
    Zoetermeer,
    Rijswijk,
    Rotterdam,
}

impl City {
    pub fn id(&self) -> CityId {
        match self {
            City::Delft => CityId(26),
            City::Eindhoven => CityId(29),
            City::DenHaag => CityId(90),
            City::Zoetermeer => CityId(6088),
            City::Rijswijk => CityId(6224),
            City::Rotterdam => CityId(25),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Holland2StayError {
    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),

    #[error("Conversion error: {0}")]
    ConversionError(String),

    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),

    #[error(transparent)]
    FromStrError(#[from] derive_more::FromStrError),
}

fn is_some_or_unknown_str<T: ToString>(option: &Option<T>) -> String {
    if let Some(t) = option {
        t.to_string()
    } else {
        "unknown".to_string()
    }
}

#[derive(derive_new::new, derive_more::Display, Hash, PartialEq, Eq)]
#[display(
    "{}: {} size: {} m2, floor: {}, minimum_stay: {}, price: {} euros, start_date: {}, contract_duration: {}, link: {}",
    city,
    name,
    is_some_or_unknown_str(size_meter_squared),
    is_some_or_unknown_str(floor),
    is_some_or_unknown_str(minimum_stay),
    is_some_or_unknown_str(price),
    is_some_or_unknown_str(start_date),
    is_some_or_unknown_str(contract_duration),
    is_some_or_unknown_str(url)
)]
pub struct House {
    pub name: String,
    pub url: Option<reqwest::Url>,
    pub city: City,
    pub size_meter_squared: Option<String>,
    pub floor: Option<String>,
    pub minimum_stay: Option<String>,
    pub price: Option<String>,
    pub start_date: Option<String>,
    pub contract_duration: Option<String>,
}

mod api_house {
    use std::collections::HashMap;

    #[derive(serde::Deserialize)]
    pub struct ApiHouse {
        pub name: String,
        pub url_key: String,
        pub living_area: Option<String>,
        pub floor: Option<serde_json::Value>,
        pub minimum_stay: Option<String>,
        pub price_range: Option<PriceRange>,
        pub next_contract_startdate: Option<String>,
        pub type_of_contract: Option<serde_json::Value>,
    }

    #[derive(serde::Deserialize)]
    pub struct PriceRange {
        pub maximum_price: Option<MaximumPrice>,
    }

    #[derive(serde::Deserialize)]
    pub struct MaximumPrice {
        pub final_price: Option<FinalPrice>,
    }

    #[derive(serde::Deserialize)]
    pub struct FinalPrice {
        pub value: Option<f64>,
    }

    #[derive(serde::Deserialize)]
    pub struct Aggregation {
        pub attribute_code: AttributeCode,
        pub options: Vec<AttributeOption>,
    }

    #[derive(serde::Deserialize)]
    pub struct AttributeOption {
        pub label: Label,
        pub value: Value,
    }
    type AttributeCode = String;
    type Label = String;
    type Value = String;

    pub type Aggregations = HashMap<AttributeCode, HashMap<Value, Label>>;
}

trait ToRustString {
    fn to_rust_string(&self) -> Option<String>;
}

impl ToRustString for serde_json::Value {
    fn to_rust_string(&self) -> Option<String> {
        match self {
            serde_json::Value::String(s) => Some(s.clone()),
            serde_json::Value::Number(n) => Some(n.to_string()),
            serde_json::Value::Bool(b) => Some(b.to_string()),
            serde_json::Value::Null => None,
            _ => None,
        }
    }
}

pub async fn query_houses_in_city(city: City) -> Result<Vec<House>, Holland2StayError> {
    let url = reqwest::Url::parse("https://api.holland2stay.com/graphql/")
        .expect("could not parse holland2stay api url");
    let client = reqwest::Client::new();
    let mut response = client
        .post(url)
        .header("User-Agent", "Mozilla/5.0")
        .header("Content-Type", "application/json")
        .body(get_graphql_query(city.id()))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;

    let conversion_error = || {
        Holland2StayError::ConversionError(
            "Could not convert json response into list of available houses".to_string(),
        )
    };

    let products = response
        .get_mut("data")
        .ok_or_else(conversion_error)?
        .get_mut("products")
        .ok_or_else(conversion_error)?;
    let mut aggregations_map: api_house::Aggregations = HashMap::new();
    {
        if let Some(aggregations) = (|| -> Option<Vec<api_house::Aggregation>> {
            Some(
                products
                    .get_mut("aggregations")?
                    .as_array_mut()?
                    .into_iter()
                    .map(|value| serde_json::from_value(value.take()))
                    .collect::<Result<_, _>>()
                    .ok()?,
            )
        })() {
            for aggregation in aggregations {
                let mut label_map = HashMap::new();
                for option in aggregation.options {
                    label_map.insert(option.value, option.label);
                }
                aggregations_map.insert(aggregation.attribute_code, label_map);
            }
        }
    }

    let api_houses: Vec<api_house::ApiHouse> = products
        .get_mut("items")
        .ok_or_else(conversion_error)?
        .as_array_mut()
        .ok_or_else(conversion_error)?
        .into_iter()
        .map(|v| serde_json::from_value(v.take()))
        .collect::<Result<_, _>>()?;

    let mut houses = Vec::new();
    for api_house in api_houses {
        let floor = || -> Option<String> {
            Some(
                aggregations_map
                    .get("floor")?
                    .get(&api_house.floor?.to_rust_string()?)?
                    .clone(),
            )
        }();
        let contract_duration = || -> Option<String> {
            Some(
                aggregations_map
                    .get("type_of_contract")?
                    .get(&api_house.type_of_contract?.to_rust_string()?)?
                    .clone(),
            )
        }();
        let price = || -> Option<String> {
            Some(
                api_house
                    .price_range?
                    .maximum_price?
                    .final_price?
                    .value?
                    .to_string(),
            )
        }();
        let start_date = || -> Option<String> {
            let naive_dt = chrono::NaiveDateTime::parse_from_str(
                &api_house.next_contract_startdate?,
                "%Y-%m-%d %H:%M:%S",
            )
            .expect("Failed to parse datetime format");
            let day = naive_dt.day();
            let month = naive_dt.format("%B"); // Full month name
            let year = naive_dt.year();

            Some(format!("{} {} {}", day, month, year))
        }();
        let url = reqwest::Url::parse("https://holland2stay.com/residences/")
            .expect("Could not parse residences url")
            .join(&api_house.url_key)
            .ok();

        let house = House::new(
            api_house.name,
            url,
            city,
            api_house.living_area,
            floor,
            api_house.minimum_stay,
            price,
            start_date,
            contract_duration,
        );
        houses.push(house);
    }
    Ok(houses)
}

pub async fn query_houses_in_cities(
    cities: impl Iterator<Item = &City>,
) -> Result<Vec<House>, Holland2StayError> {
    let future_houses = cities.map(async |&city| query_houses_in_city(city).await);

    futures::future::join_all(future_houses)
        .await
        .into_iter()
        .try_fold(
            vec![],
            |mut acc, houses| -> Result<Vec<House>, Holland2StayError> {
                acc.append(&mut houses?);
                Ok(acc)
            },
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_body() {
        let body = get_graphql_query(City::Eindhoven.id());
        println!("{}", body);
    }

    #[tokio::test]
    async fn test_query_houses_in_city() {
        let houses = query_houses_in_city(City::Rotterdam).await.unwrap();
        for house in houses {
            println!("{}", house);
        }
    }

    #[tokio::test]
    async fn test_query_houses_cities() {
        let cities = query_houses_in_cities(
            [City::Rotterdam, City::Eindhoven, City::DenHaag, City::Delft].iter(),
        )
        .await
        .unwrap();
        for city in cities {
            println!("{}", city);
        }
    }
}
