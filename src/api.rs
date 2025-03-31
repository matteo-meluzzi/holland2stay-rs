fn get_graphql_query(city_id: u64) -> String {
    format!(
        r#"{{ "operationName": "GetCategories", "variables": {{ "currentPage": 1, "filters": {{ "available_to_book": {{ "eq": "179" }}, "category_uid": {{ "eq": "Nw==" }}, "city": {{ "eq": "{}" }} }}, "pageSize": 100, "sort": {{ "available_startdate": "ASC" }} }}, "query": "query GetCategories($pageSize: Int!, $currentPage: Int!, $filters: ProductAttributeFilterInput!, $sort: ProductAttributeSortInput) {{ products( pageSize: $pageSize, currentPage: $currentPage, filter: $filters, sort: $sort ) {{ ...ProductsFragment, __typename }} }} fragment ProductsFragment on Products {{ sort_fields {{ options {{ label, value, __typename }}, __typename }}, aggregations {{ label, count, attribute_code, options {{ label, count, value, __typename }}, position, __typename }}, items {{ name, sku, city, url_key, available_to_book, available_startdate, next_contract_startdate, current_lottery_subscribers, building_name, finishing, living_area, no_of_rooms, resident_type, offer_text_two, offer_text, maximum_number_of_persons, type_of_contract, price_analysis_text, allowance_price, floor, basic_rent, lumpsum_service_charge, inventory, caretaker_costs, cleaning_common_areas, energy_common_areas, energy_label, minimum_stay, allowance_price, price_range {{ minimum_price {{ regular_price {{ value, currency, __typename }}, final_price {{ value, currency, __typename }}, __typename }}, maximum_price {{ regular_price {{ value, currency, __typename }}, final_price {{ value, currency, __typename }}, __typename }}, __typename }} , __typename }}, total_count, __typename }}" }}"#,
        city_id
    )
}

pub async fn query_rooms() -> Result<(), reqwest::Error> {
    let url = reqwest::Url::parse("https://api.holland2stay.com/graphql/")
        .expect("could not parse holland2stay api url");
    let client = reqwest::Client::new();
    let response = client
        .post(url)
        .header("User-Agent", "Mozilla/5.0")
        .header("Content-Type", "application/json")
        .body(get_graphql_query(29))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;

    fn get_rooms(data: &serde_json::Value) -> Option<Vec<&str>> {
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
                .collect(),
        )
    }

    let rooms = get_rooms(&response).ok_or_else(|| todo!())?;
    for room in rooms {
        println!("{room}");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_body() {
        let body = get_graphql_query(29);
        println!("{}", body);
    }

    #[tokio::test]
    async fn test_query_rooms() {
        query_rooms().await.unwrap();
    }
}
