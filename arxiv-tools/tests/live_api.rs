//! Tests that talk to the real arXiv API.
//!
//! Parsing and URL building are covered by the offline unit tests in `src/`;
//! these check that the request we build is one arXiv actually accepts, and
//! that the shapes we parse still match what it sends.
//!
//! They need network access and are therefore slower and less reliable than
//! the rest of the suite. Every request goes through the shared client, which
//! spaces requests three seconds apart per the arXiv Terms of Use, so the file
//! takes a while to run by design.
//!
//! Run only the offline tests with `cargo test --lib`.
//!
//! arXiv enforces a volume limit on top of the three-second spacing this
//! crate keeps, and running this file repeatedly in quick succession will earn
//! a `429 Rate exceeded` that lasts several minutes. That shows up here as
//! `Error::Status { status: 429, .. }`. Wait it out rather than retrying
//! immediately, and prefer `cargo test --lib` while iterating.

use arxiv_tools::{ArXiv, Category, Error, Paper, QueryParams, SortBy, SortOrder};

/// "Attention Is All You Need".
const TRANSFORMER: &str = "1706.03762";
/// "BERT: Pre-training of Deep Bidirectional Transformers...".
const BERT: &str = "1810.04805";
/// The oldest paper on arXiv with a journal reference and a DOI.
const DIPHOTON: &str = "0704.0001";

#[tokio::test]
async fn title_search_finds_the_paper() {
    let papers = ArXiv::from_args(QueryParams::title("attention is all you need"))
        .max_results(5)
        .query()
        .await
        .unwrap();

    assert!(!papers.is_empty());
    assert!(
        papers
            .iter()
            .any(|p| p.title.to_lowercase().contains("attention is all you need"))
    );
}

#[tokio::test]
async fn author_search_finds_the_author() {
    let papers = ArXiv::from_args(QueryParams::author("Yoshua Bengio"))
        .max_results(5)
        .query()
        .await
        .unwrap();

    assert!(!papers.is_empty());
    assert!(papers.iter().any(|p| {
        p.authors
            .iter()
            .any(|a| a.to_lowercase().contains("bengio"))
    }));
}

#[tokio::test]
async fn id_list_fetches_exactly_the_requested_papers() {
    let papers = ArXiv::from_id_list([TRANSFORMER, BERT])
        .query()
        .await
        .unwrap();

    assert_eq!(papers.len(), 2);
    assert!(papers.iter().any(|p| p.id.contains(TRANSFORMER)));
    assert!(papers.iter().any(|p| p.id.contains(BERT)));

    let transformer = papers.iter().find(|p| p.id.contains(TRANSFORMER)).unwrap();
    assert_eq!(transformer.title, "Attention Is All You Need");
    assert_eq!(transformer.authors.len(), 8);
    assert_eq!(transformer.primary_category, "cs.CL");
    assert!(transformer.pdf_url.contains(TRANSFORMER));
    assert!(transformer.published2utc().is_ok());
    assert!(transformer.arxiv_id().starts_with(TRANSFORMER));
}

#[tokio::test]
async fn journal_ref_and_doi_come_back_populated() {
    // Regression: 1.x returned "\n  " for every journal_ref, and put the DOI
    // resolver URL in `doi` rather than the DOI itself.
    let papers = ArXiv::from_id_list([DIPHOTON]).query().await.unwrap();

    let paper = &papers[0];
    assert_eq!(paper.journal_ref, "Phys.Rev.D76:013009,2007");
    assert_eq!(paper.doi, "10.1103/PhysRevD.76.013009");
    assert_eq!(paper.doi_url, "https://doi.org/10.1103/PhysRevD.76.013009");
}

#[tokio::test]
async fn a_query_with_no_matches_returns_no_papers() {
    let papers = ArXiv::from_args(QueryParams::title(
        "xyzzy123qwertyuiop456asdfghjkl789zxcvbnm",
    ))
    .query()
    .await
    .unwrap();

    assert!(papers.is_empty());
}

#[tokio::test]
async fn a_rejected_query_surfaces_as_an_api_error() {
    // 1.x parsed arXiv's error feed as a normal result and returned it as a
    // Paper titled "Error".
    let err = ArXiv::from_id_list(["not-an-arxiv-id"])
        .query()
        .await
        .unwrap_err();

    match err {
        Error::Api { message } => assert!(message.contains("incorrect id format"), "{message}"),
        other => panic!("expected Error::Api, got {other:?}"),
    }
}

#[tokio::test]
async fn a_grouped_boolean_query_is_accepted_by_the_api() {
    // Regression: 1.x emitted (cat:"cs.AI"cat:"cs.LG") for a multi-operand
    // group, which arXiv could not parse.
    let args = QueryParams::and(vec![
        QueryParams::group(vec![QueryParams::or(vec![
            QueryParams::subject_category(Category::CsAi),
            QueryParams::subject_category(Category::CsLg),
        ])]),
        QueryParams::submitted_date("202412010000", "202412012359"),
    ]);

    let papers = ArXiv::from_args(args)
        .max_results(20)
        .query()
        .await
        .unwrap();

    assert!(!papers.is_empty());
    for paper in &papers {
        assert!(
            paper
                .categories
                .iter()
                .any(|c| c == "cs.AI" || c == "cs.LG"),
            "{} has categories {:?}",
            paper.id,
            paper.categories
        );
    }
}

#[tokio::test]
async fn an_implicitly_grouped_boolean_query_means_what_the_tree_says() {
    // Without automatic parenthesisation this rendered as
    // `cat:"cs.AI" OR cat:"cs.LG" AND submittedDate:[...]`, whose meaning
    // depended on arXiv's operator precedence rather than on the tree.
    let args = QueryParams::and(vec![
        QueryParams::or(vec![
            QueryParams::subject_category(Category::CsAi),
            QueryParams::subject_category(Category::CsLg),
        ]),
        QueryParams::submitted_date("202412010000", "202412012359"),
    ]);

    let papers = ArXiv::from_args(args)
        .max_results(20)
        .query()
        .await
        .unwrap();

    assert!(!papers.is_empty());
    for paper in &papers {
        // The AND must still bind the date range, so every hit is from that
        // one day as well as being in one of the two categories.
        let published = paper.published2utc().unwrap();
        assert!(
            paper
                .categories
                .iter()
                .any(|c| c == "cs.AI" || c == "cs.LG"),
            "{} has categories {:?}",
            paper.id,
            paper.categories
        );
        assert_eq!(
            published.format("%Y-%m").to_string(),
            "2024-12",
            "{} was published {published}, so the date range did not bind",
            paper.id
        );
    }
}

#[tokio::test]
async fn a_category_added_in_2_0_is_queryable() {
    // stat.ML did not exist in the 1.x Category enum.
    let papers = ArXiv::from_args(QueryParams::subject_category(Category::StatMl))
        .max_results(5)
        .query()
        .await
        .unwrap();

    assert!(!papers.is_empty());
}

#[tokio::test]
async fn results_honour_the_requested_sort_order() {
    let papers = ArXiv::from_args(QueryParams::subject_category(Category::CsCl))
        .max_results(20)
        .sort_by(SortBy::SubmittedDate)
        .sort_order(SortOrder::Descending)
        .query()
        .await
        .unwrap();

    assert!(papers.len() > 1);
    for pair in papers.windows(2) {
        let newer = pair[0].published2utc().unwrap();
        let older = pair[1].published2utc().unwrap();
        assert!(newer >= older, "results are not sorted descending");
    }
}

#[tokio::test]
async fn query_page_reports_the_paging_counters() {
    let page = ArXiv::from_args(QueryParams::subject_category(Category::CsLg))
        .start(10)
        .max_results(5)
        .query_page()
        .await
        .unwrap();

    assert_eq!(page.papers.len(), 5);
    assert_eq!(page.start_index, 10);
    assert!(page.total_results > 1000, "got {}", page.total_results);
}

#[tokio::test]
async fn query_all_pages_past_a_single_request() {
    let papers = ArXiv::from_args(QueryParams::subject_category(Category::CsLg))
        .max_results(60) // page size
        .query_all(Some(150))
        .await
        .unwrap();

    assert_eq!(papers.len(), 150);

    let mut ids: Vec<&str> = papers.iter().map(|p| p.id.as_str()).collect();
    ids.sort_unstable();
    let total = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), total, "query_all returned duplicate papers");
}

#[tokio::test]
async fn pdfs_can_be_downloaded_to_disk() {
    let papers = ArXiv::from_id_list([TRANSFORMER]).query().await.unwrap();
    let paper = &papers[0];

    let dir = std::env::temp_dir().join("arxiv-tools-live-pdf");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{}.pdf", paper.arxiv_id()));
    let _ = std::fs::remove_file(&path);

    let written = paper.download_pdf_to(&path).await.unwrap();

    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(bytes.len() as u64, written);
    assert!(written > 100_000, "suspiciously small PDF: {written} bytes");
    assert_eq!(&bytes[..5], b"%PDF-", "downloaded file is not a PDF");

    let leftovers: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name())
        .filter(|name| name.to_string_lossy().ends_with(".part"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "staging files left behind: {leftovers:?}"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn concurrent_downloads_to_one_path_do_not_corrupt_each_other() {
    // Every attempt used to stage into the same `<destination>.part`, so two
    // downloads racing on one destination truncated and interleaved into the
    // same file. Whichever finishes last should win, intact.
    let papers = ArXiv::from_id_list([TRANSFORMER, BERT])
        .query()
        .await
        .unwrap();

    let dir = std::env::temp_dir().join("arxiv-tools-live-race");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("contended.pdf");
    let _ = std::fs::remove_file(&path);

    let downloads = papers.into_iter().map(|paper| {
        let path = path.clone();
        tokio::spawn(async move { paper.download_pdf_to(&path).await })
    });

    let mut sizes = Vec::new();
    for handle in downloads.collect::<Vec<_>>() {
        sizes.push(handle.await.unwrap().unwrap());
    }

    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(&bytes[..5], b"%PDF-", "the contended file is not a PDF");
    assert!(
        sizes.contains(&(bytes.len() as u64)),
        "the file is {} bytes, which matches neither download {sizes:?}",
        bytes.len()
    );

    // No staging files were left behind.
    let leftovers: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name())
        .filter(|name| name.to_string_lossy().ends_with(".part"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "staging files left behind: {leftovers:?}"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn downloading_a_paper_with_no_pdf_url_is_rejected() {
    let err = Paper::default().download_pdf().await.unwrap_err();
    assert!(matches!(err, Error::InvalidParam(_)), "got {err:?}");
}
