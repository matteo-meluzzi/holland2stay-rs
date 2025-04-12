use std::{collections::HashMap, rc::Rc, str::FromStr};

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

#[derive(derive_new::new, derive_more::Display, Hash, PartialEq, Eq)]
#[display(
    "{}: {} size: {} m2, floor: {}, minimum_stay: {}, price: {} euros, start_date: {}, contract_duration: {}",
    city,
    name,
    size_meter_squared,
    floor,
    minimum_stay,
    price,
    start_date,
    contract_duration
)]
pub struct House {
    pub name: String,
    pub city: City,
    pub size_meter_squared: String,
    pub floor: String,
    pub minimum_stay: String,
    pub price: String,
    pub start_date: String,
    pub contract_duration: String,
}

mod api_house {
    use std::collections::HashMap;

    #[derive(serde::Deserialize)]
    pub struct ApiHouse {
        pub name: String,
        pub city: serde_json::Value,
        pub living_area: String,
        pub floor: serde_json::Value,
        pub minimum_stay: String,
        pub price_range: PriceRange,
        pub next_contract_startdate: String,
        pub type_of_contract: serde_json::Value,
    }

    #[derive(serde::Deserialize)]
    pub struct PriceRange {
        pub maximum_price: MaximumPrice,
    }

    #[derive(serde::Deserialize)]
    pub struct MaximumPrice {
        pub final_price: FinalPrice,
    }

    #[derive(serde::Deserialize)]
    pub struct FinalPrice {
        pub value: f64,
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
    fn to_rust_string(&self) -> String;
}

impl ToRustString for serde_json::Value {
    fn to_rust_string(&self) -> String {
        match self {
            serde_json::Value::String(s) => s.clone(), // Return raw string without quotes
            serde_json::Value::Number(n) => n.to_string(), // Convert numbers directly
            serde_json::Value::Bool(b) => b.to_string(), // Convert booleans directly
            serde_json::Value::Null => "null".to_string(), // Convert null to "null"
            _ => "object".to_string(),                 // Serialize complex types normally
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
        .ok_or(conversion_error())?
        .get_mut("products")
        .ok_or(conversion_error())?;
    let mut aggregations_map: api_house::Aggregations = HashMap::new();
    {
        let aggregations: Vec<api_house::Aggregation> = products
            .get_mut("aggregations")
            .ok_or(conversion_error())?
            .as_array_mut()
            .ok_or(conversion_error())?
            .into_iter()
            .map(|value| serde_json::from_value(value.take()))
            .collect::<Result<_, _>>()?;
        for aggregation in aggregations {
            let mut label_map = HashMap::new();
            for option in aggregation.options {
                label_map.insert(option.value, option.label);
            }
            aggregations_map.insert(aggregation.attribute_code, label_map);
        }
    }

    let api_houses: Vec<api_house::ApiHouse> = products
        .get_mut("items")
        .ok_or(conversion_error())?
        .as_array_mut()
        .ok_or(conversion_error())?
        .into_iter()
        .map(|v| serde_json::from_value(v.take()))
        .collect::<Result<_, _>>()?;

    let mut houses = Vec::new();
    for api_house in api_houses {
        let api_house_error =
            || Holland2StayError::ConversionError("Could not perform aggregation".to_string());

        let floor = aggregations_map
            .get("floor")
            .ok_or_else(api_house_error)?
            .get(&api_house.floor.to_rust_string())
            .ok_or_else(api_house_error)?
            .clone();
        let contract_duration = aggregations_map
            .get("type_of_contract")
            .ok_or_else(api_house_error)?
            .get(&api_house.type_of_contract.to_rust_string())
            .ok_or_else(api_house_error)?
            .clone();

        let house = House::new(
            api_house.name,
            city,
            api_house.living_area,
            floor,
            api_house.minimum_stay,
            api_house
                .price_range
                .maximum_price
                .final_price
                .value
                .to_string(),
            api_house.next_contract_startdate,
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
