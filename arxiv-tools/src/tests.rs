use super::*;

#[tokio::test]
async fn test_no_such_a_paper() {
    let mut arxiv =
        ArXiv::from_args(QueryParams::title("xyzzy123qwertyuiop456asdfghjkl789zxcvbnm"));
    let response = arxiv.query().await.unwrap();
    assert_eq!(response.len(), 0);
}

#[tokio::test]
async fn test_query_by_author() {
    let mut arxiv = ArXiv::from_args(QueryParams::author("Yoshua Bengio"));
    arxiv.max_results(5);

    let response = arxiv.query().await.unwrap();
    assert!(response.len() > 0);

    let has_author = response.iter().any(|paper| {
        paper
            .authors
            .iter()
            .any(|author| author.to_lowercase().contains("bengio"))
    });
    assert!(has_author);
}

#[tokio::test]
async fn test_query_by_abstract() {
    let mut arxiv = ArXiv::from_args(QueryParams::abstract_text("deep learning"));
    arxiv.max_results(5);

    let response = arxiv.query().await.unwrap();
    assert!(response.len() > 0);
}

#[tokio::test]
async fn test_query_by_id_list_single() {
    // Query a specific paper by arXiv ID using id_list
    let mut arxiv = ArXiv::from_id_list(vec!["1706.03762"]);

    let response = arxiv.query().await.unwrap();
    assert!(response.len() > 0);

    let paper = response.first().unwrap();
    assert!(paper.id.contains("1706.03762"));
    assert!(paper
        .title
        .to_lowercase()
        .contains("attention is all you need"));
}

#[tokio::test]
async fn test_query_by_id_list_multiple() {
    // Query multiple papers by arXiv IDs
    // 1706.03762: "Attention Is All You Need"
    // 1810.04805: "BERT"
    let mut arxiv = ArXiv::from_id_list(vec!["1706.03762", "1810.04805"]);

    let response = arxiv.query().await.unwrap();
    assert_eq!(response.len(), 2);

    let ids: Vec<&str> = response.iter().map(|p| p.id.as_str()).collect();
    assert!(ids.iter().any(|id| id.contains("1706.03762")));
    assert!(ids.iter().any(|id| id.contains("1810.04805")));
}

#[tokio::test]
async fn test_query_by_id_list_method() {
    // Test using id_list() method on existing ArXiv instance
    let mut arxiv = ArXiv::from_args(QueryParams::default());
    arxiv.id_list(vec!["1706.03762"]);

    let response = arxiv.query().await.unwrap();
    assert!(response.len() > 0);

    let paper = response.first().unwrap();
    assert!(paper.id.contains("1706.03762"));
}

#[tokio::test]
async fn test_query_by_all() {
    let mut arxiv = ArXiv::from_args(QueryParams::all("transformer"));
    arxiv.max_results(5);

    let response = arxiv.query().await.unwrap();
    assert!(response.len() > 0);
}

#[tokio::test]
async fn test_query_and_not() {
    let args = QueryParams::and_not(vec![
        QueryParams::title("neural network"),
        QueryParams::title("deep learning"),
    ]);
    let mut arxiv = ArXiv::from_args(args);
    arxiv.max_results(5);

    let response = arxiv.query().await.unwrap();
    assert!(response.len() > 0);
}

#[tokio::test]
async fn test_query_simple() {
    let mut arxiv = ArXiv::from_args(QueryParams::title("attention is all you need"));

    let url = arxiv.build_query();
    println!("{}", url);

    let response = arxiv.query().await.unwrap();
    assert!(response.len() > 0);

    let response = serde_json::to_string_pretty(&response).unwrap();
    println!("{:?}", response);
}

#[tokio::test]
async fn test_query_normal() {
    let args = QueryParams::and(vec![
        QueryParams::or(vec![
            QueryParams::subject_category(Category::CsAi),
            QueryParams::subject_category(Category::CsLg),
        ]),
        QueryParams::SubmittedDate(String::from("202412010000"), String::from("202412012359")),
    ]);
    let mut arxiv = ArXiv::from_args(args);

    let url = arxiv.build_query();
    println!("{}", url);

    let response = arxiv.query().await.unwrap();
    assert!(response.len() > 0);

    response.iter().for_each(|paper| {
        let published = paper.published2utc();
        assert!(
            DateTime::parse_from_rfc3339("2024-12-01T00:00:00Z").unwrap() <= published
                && published <= DateTime::parse_from_rfc3339("2024-12-01T23:59:00Z").unwrap()
        );
    });

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
        QueryParams::SubmittedDate(String::from("202412010000"), String::from("202412012359")),
    ]);
    let mut arxiv = ArXiv::from_args(args);
    arxiv.start(10);
    arxiv.max_results(100);
    arxiv.sort_by(SortBy::SubmittedDate);
    arxiv.sort_order(SortOrder::Ascending);

    let url = arxiv.build_query();
    println!("{}", url);

    let response = arxiv.query().await.unwrap();
    println!("{:?}", response);
    assert!(response.len() > 0);

    let response = serde_json::to_string_pretty(&response.first().unwrap()).unwrap();
    println!("{}", response);
}

#[tokio::test]
async fn test_paper_updated2utc() {
    // Use id_list to get a specific paper
    let mut arxiv = ArXiv::from_id_list(vec!["1706.03762"]);

    let response = arxiv.query().await.unwrap();
    assert!(response.len() > 0);

    let paper = response.first().unwrap();
    let updated = paper.updated2utc();
    let published = paper.published2utc();

    // updated should be >= published
    assert!(updated >= published);
}

#[tokio::test]
async fn test_sort_by_last_updated_date() {
    let args = QueryParams::subject_category(Category::CsAi);
    let mut arxiv = ArXiv::from_args(args);
    arxiv.max_results(10);
    arxiv.sort_by(SortBy::LastUpdatedDate);
    arxiv.sort_order(SortOrder::Descending);

    let response = arxiv.query().await.unwrap();
    assert!(response.len() > 0);

    // Verify results are sorted by last updated date (descending)
    for i in 0..response.len() - 1 {
        let current = response[i].updated2utc();
        let next = response[i + 1].updated2utc();
        assert!(current >= next, "Results should be sorted by lastUpdatedDate descending");
    }
}

#[tokio::test]
async fn test_sort_by_relevance() {
    let mut arxiv = ArXiv::from_args(QueryParams::title("machine learning"));
    arxiv.max_results(5);
    arxiv.sort_by(SortBy::Relevance);

    let response = arxiv.query().await.unwrap();
    assert!(response.len() > 0);
}

#[tokio::test]
async fn test_sort_order_descending() {
    let args = QueryParams::subject_category(Category::CsCl);
    let mut arxiv = ArXiv::from_args(args);
    arxiv.max_results(10);
    arxiv.sort_by(SortBy::SubmittedDate);
    arxiv.sort_order(SortOrder::Descending);

    let response = arxiv.query().await.unwrap();
    assert!(response.len() > 0);

    // Verify results are sorted by submitted date (descending)
    for i in 0..response.len() - 1 {
        let current = response[i].published2utc();
        let next = response[i + 1].published2utc();
        assert!(current >= next, "Results should be sorted by submittedDate descending");
    }
}

#[test]
fn test_category_to_string() {
    assert_eq!(Category::CsAi.to_string(), "cs.AI");
    assert_eq!(Category::CsCl.to_string(), "cs.CL");
    assert_eq!(Category::CsLg.to_string(), "cs.LG");
    assert_eq!(Category::CsGt.to_string(), "cs.GT");
    assert_eq!(Category::CsCv.to_string(), "cs.CV");
    assert_eq!(Category::CsCr.to_string(), "cs.CR");
    assert_eq!(Category::CsCc.to_string(), "cs.CC");
    assert_eq!(Category::CsCe.to_string(), "cs.CE");
    assert_eq!(Category::CsCy.to_string(), "cs.CY");
    assert_eq!(Category::CsDs.to_string(), "cs.DS");
    assert_eq!(Category::CsDm.to_string(), "cs.DM");
    assert_eq!(Category::CsDc.to_string(), "cs.DC");
    assert_eq!(Category::CsEt.to_string(), "cs.ET");
    assert_eq!(Category::CsFl.to_string(), "cs.FL");
    assert_eq!(Category::CsGl.to_string(), "cs.GL");
    assert_eq!(Category::CsGr.to_string(), "cs.GR");
    assert_eq!(Category::CsAr.to_string(), "cs.AR");
    assert_eq!(Category::CsHc.to_string(), "cs.HC");
    assert_eq!(Category::CsIr.to_string(), "cs.IR");
}

#[test]
fn test_sort_by_to_string() {
    assert_eq!(SortBy::Relevance.to_string(), "relevance");
    assert_eq!(SortBy::LastUpdatedDate.to_string(), "lastUpdatedDate");
    assert_eq!(SortBy::SubmittedDate.to_string(), "submittedDate");
}

#[test]
fn test_sort_order_to_string() {
    assert_eq!(SortOrder::Ascending.to_string(), "ascending");
    assert_eq!(SortOrder::Descending.to_string(), "descending");
}

#[test]
fn test_query_params_default() {
    let params = QueryParams::default();
    assert_eq!(params.to_string(), "ti:\"default\"");
}

#[test]
fn test_paper_default() {
    let paper = Paper::default();
    assert_eq!(paper.id, "");
    assert_eq!(paper.title, "");
    assert!(paper.authors.is_empty());
    assert_eq!(paper.abstract_text, "");
    assert_eq!(paper.published, "");
    assert_eq!(paper.updated, "");
    assert_eq!(paper.doi, "");
    assert!(paper.comment.is_empty());
    assert_eq!(paper.journal_ref, "");
    assert_eq!(paper.pdf_url, "");
    assert_eq!(paper.primary_category, "");
    assert!(paper.categories.is_empty());
}
