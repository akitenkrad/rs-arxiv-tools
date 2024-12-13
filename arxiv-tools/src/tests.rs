use super::*;

#[tokio::test]
async fn test_query_simple() {
    let mut arxiv = ArXiv::new();
    arxiv.title("attention is all you need");
    let response = arxiv.query().await;
    assert!(response.len() > 0);

    let response = serde_json::to_string_pretty(&response).unwrap();
    println!("{:?}", response);
}

#[tokio::test]
async fn test_query_normal() {
    let mut arxiv = ArXiv::new();
    arxiv
        .subject_category("cs.AI")
        .or()
        .subject_category("cs.LG")
        .submitted_date("202412010000", "202412012359");
    let response = arxiv.query().await;
    assert!(response.len() > 0);

    let response = serde_json::to_string_pretty(&response).unwrap();
    println!("{}", response);
}

#[tokio::test]
async fn test_query_complex() {
    let mut arxiv = ArXiv::new();
    arxiv
        .title("ai")
        .or()
        .title("llm")
        .and()
        .group_start()
        .subject_category("cs.AI")
        .or()
        .subject_category("cs.LG")
        .group_end()
        .submitted_date("202412010000", "202412012359");
    let response = arxiv.query().await;
    assert!(response.len() > 0);

    let response = serde_json::to_string_pretty(&response).unwrap();
    println!("{}", response);
}
