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
}

#[derive(derive_new::new, derive_more::Display, Hash, PartialEq, Eq)]
#[display("{}: {}", city, name)]
pub struct House {
    pub name: String,
    pub city: City,
}

pub async fn query_houses_in_city(city: City) -> Result<Vec<House>, Holland2StayError> {
    let url = reqwest::Url::parse("https://api.holland2stay.com/graphql/")
        .expect("could not parse holland2stay api url");
    let client = reqwest::Client::new();
    let response = client
        .post(url)
        .header("User-Agent", "Mozilla/5.0")
        .header("Content-Type", "application/json")
        .body(get_graphql_query(city.id()))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;

    fn get_rooms(data: &serde_json::Value, city: City) -> Option<Vec<House>> {
        Some(
            data.as_object()?
                .get("data")?
                .as_object()?
                .get("products")?
                .as_object()?
                .get("items")?
                .as_array()?
                .into_iter()
                .flat_map(|v| v.as_object()?.get("name")?.as_str())
                .map(|s| House::new(s.to_string(), city))
                .collect(),
        )
    }

    get_rooms(&response, city).ok_or_else(|| {
        Holland2StayError::ConversionError(
            "Could not convert json response into list of available houses".to_string(),
        )
    })
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
        let cities = query_houses_in_city(City::Rotterdam).await.unwrap();
        for city in cities {
            println!("{}", city.name);
        }
    }

    #[tokio::test]
    async fn test_query_houses_cities() {
        let cities = query_houses_in_cities(&[City::Rotterdam, City::Eindhoven])
            .await
            .unwrap();
        for city in cities {
            println!("{}", city);
        }
    }
}
