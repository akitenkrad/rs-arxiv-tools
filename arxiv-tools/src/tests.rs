use super::*;

#[tokio::test]
async fn test_no_such_a_paper() {
    let mut arxiv = ArXiv::from_args(QueryParams::title("there is no such a paper"));
    let response = arxiv.query().await;
    assert_eq!(response.len(), 0);
}

#[tokio::test]
async fn test_query_simple() {
    let mut arxiv = ArXiv::from_args(QueryParams::title("attention is all you need"));
    let response = arxiv.query().await;
    assert!(response.len() > 0);

    let response = serde_json::to_string_pretty(&response).unwrap();
    println!("{:?}", response);
}

#[tokio::test]
async fn test_query_normal() {
    let args = QueryParams::and(vec![
        QueryParams::subject_category(Category::CsAi),
        QueryParams::subject_category(Category::CsLg),
    ]);
    let mut arxiv = ArXiv::from_args(args);
    arxiv.submitted_date("202412010000", "202412012359");
    let response = arxiv.query().await;
    assert!(response.len() > 0);

    let response = serde_json::to_string_pretty(&response).unwrap();
    println!("{}", response);
}

#[tokio::test]
async fn test_query_complex() {
    let args = QueryParams::and(vec![
        QueryParams::or(vec![QueryParams::title("ai"), QueryParams::title("llm")]),
        QueryParams::group(vec![QueryParams::or(vec![
            QueryParams::subject_category(Category::CsAi),
            QueryParams::subject_category(Category::CsLg),
        ])]),
    ]);
    let mut arxiv = ArXiv::from_args(args);
    arxiv.submitted_date("202412010000", "202412012359");
    arxiv.start(10);
    arxiv.max_results(100);
    arxiv.sort_by(SortBy::SubmittedDate);
    arxiv.sort_order(SortOrder::Ascending);
    let response = arxiv.query().await;
    assert!(response.len() > 1);

    let response = serde_json::to_string_pretty(&response.first().unwrap()).unwrap();
    println!("{}", response);
}
