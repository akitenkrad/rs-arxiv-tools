use super::*;

#[tokio::test]
async fn test_no_such_a_paper() {
    let mut arxiv = ArXiv::from_args(ArXivArgs::title("there is no such a paper"));
    let response = arxiv.query().await;
    assert_eq!(response.len(), 0);
}

#[tokio::test]
async fn test_query_simple() {
    let mut arxiv = ArXiv::from_args(ArXivArgs::title("attention is all you need"));
    let response = arxiv.query().await;
    assert!(response.len() > 0);

    let response = serde_json::to_string_pretty(&response).unwrap();
    println!("{:?}", response);
}

#[tokio::test]
async fn test_query_normal() {
    let args = ArXivArgs::and(vec![
        ArXivArgs::subject_category(ArXivCategory::CsAi),
        ArXivArgs::subject_category(ArXivCategory::CsLg),
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
    let args = ArXivArgs::and(vec![
        ArXivArgs::or(vec![ArXivArgs::title("ai"), ArXivArgs::title("llm")]),
        ArXivArgs::group(vec![ArXivArgs::or(vec![
            ArXivArgs::subject_category(ArXivCategory::CsAi),
            ArXivArgs::subject_category(ArXivCategory::CsLg),
        ])]),
    ]);
    let mut arxiv = ArXiv::from_args(args);
    arxiv.submitted_date("202412010000", "202412012359");
    let response = arxiv.query().await;
    assert!(response.len() > 0);

    let response = serde_json::to_string_pretty(&response).unwrap();
    println!("{}", response);
}
