//! # Description
//! This library provides a simple interface to query the arXiv API.
//!
//! # Example
//! ## Simple Query
//! ```rust
//! # use arxiv_tools::{ArXiv, QueryParams, Paper};
//! # #[tokio::main]
//! # async fn main() {
//! // get arxiv object from query parameters
//! let mut arxiv = ArXiv::from_args(QueryParams::title("attention is all you need"));
//!
//! // execute
//! let response: Vec<Paper> = arxiv.query().await;
//!
//! //verify
//! let paper = response.first().unwrap();
//! assert!(paper.title.to_lowercase().contains("attention is all you need"));
//! # }
//! ```
//!
//! ## Complex Query
//! ```rust
//! # use arxiv_tools::{ArXiv, QueryParams, Category, SortBy, SortOrder};
//! # #[tokio::main]
//! # async fn main() {
//! // build query parameters
//! let args = QueryParams::and(vec![
//!     QueryParams::or(vec![QueryParams::title("ai"), QueryParams::title("llm")]),
//!     QueryParams::group(vec![QueryParams::or(vec![
//!         QueryParams::subject_category(Category::CsAi),
//!         QueryParams::subject_category(Category::CsLg),
//!     ])]),
//!     QueryParams::SubmittedDate(String::from("202412010000"), String::from("202412012359")),
//! ]);
//! let mut arxiv = ArXiv::from_args(args);
//!
//! // set additional parameters
//! arxiv.start(10);
//! arxiv.max_results(100);
//! arxiv.sort_by(SortBy::SubmittedDate);
//! arxiv.sort_order(SortOrder::Ascending);
//!
//! // execute
//! let response = arxiv.query().await;
//!
//! // verify
//! assert!(response.len() > 0);
//! # }
//! ```
use chrono::{DateTime, Utc};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use reqwest as request;
use serde::{Deserialize, Serialize};
use urlencoding::encode;

pub enum Category {
    CsAi,
    CsCl,
    CsLg,
    CsGt,
    CsCv,
    CsCr,
    CsCc,
    CsCe,
    CsCy,
    CsDs,
    CsDm,
    CsDc,
    CsEt,
    CsFl,
    CsGl,
    CsGr,
    CsAr,
    CsHc,
    CsIr,
}

impl Category {
    pub fn to_string(&self) -> String {
        match self {
            Category::CsAi => String::from("cs.AI"),
            Category::CsCl => String::from("cs.CL"),
            Category::CsLg => String::from("cs.LG"),
            Category::CsGt => String::from("cs.GT"),
            Category::CsCv => String::from("cs.CV"),
            Category::CsCr => String::from("cs.CR"),
            Category::CsCc => String::from("cs.CC"),
            Category::CsCe => String::from("cs.CE"),
            Category::CsCy => String::from("cs.CY"),
            Category::CsDs => String::from("cs.DS"),
            Category::CsDm => String::from("cs.DM"),
            Category::CsDc => String::from("cs.DC"),
            Category::CsEt => String::from("cs.ET"),
            Category::CsFl => String::from("cs.FL"),
            Category::CsGl => String::from("cs.GL"),
            Category::CsGr => String::from("cs.GR"),
            Category::CsAr => String::from("cs.AR"),
            Category::CsHc => String::from("cs.HC"),
            Category::CsIr => String::from("cs.IR"),
        }
    }
}

#[derive(Clone, Debug)]
pub enum QueryParams {
    Title(String),
    Author(String),
    Abstract(String),
    Comment(String),
    JournalRef(String),
    SubjectCategory(String),
    ReportNumber(String),
    Id(String),
    All(String),
    And(String),
    Or(String),
    AndNot(String),
    Group(String),
    SubmittedDate(String, String),
}

impl Default for QueryParams {
    fn default() -> Self {
        return QueryParams::title("default");
    }
}

#[derive(Clone, Debug, Default)]
pub enum SortBy {
    #[default]
    Relevance,
    LastUpdatedDate,
    SubmittedDate,
}

impl SortBy {
    pub fn to_string(&self) -> String {
        match self {
            SortBy::Relevance => String::from("relevance"),
            SortBy::LastUpdatedDate => String::from("lastUpdatedDate"),
            SortBy::SubmittedDate => String::from("submittedDate"),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub enum SortOrder {
    #[default]
    Ascending,
    Descending,
}

impl SortOrder {
    pub fn to_string(&self) -> String {
        match self {
            SortOrder::Ascending => String::from("ascending"),
            SortOrder::Descending => String::from("descending"),
        }
    }
}

impl QueryParams {
    pub fn title(arg: &str) -> Self {
        return QueryParams::Title(format!("ti:\"{}\"", encode(arg)));
    }
    pub fn author(arg: &str) -> Self {
        return QueryParams::Author(format!("au:\"{}\"", encode(arg)));
    }
    pub fn abstract_text(arg: &str) -> Self {
        return QueryParams::Abstract(format!("abs:\"{}\"", encode(arg)));
    }
    pub fn comment(arg: &str) -> Self {
        return QueryParams::Comment(format!("co:\"{}\"", encode(arg)));
    }
    pub fn journal_ref(arg: &str) -> Self {
        return QueryParams::JournalRef(format!("jr:\"{}\"", encode(arg)));
    }
    pub fn subject_category(arg: Category) -> Self {
        return QueryParams::SubjectCategory(format!("cat:\"{}\"", encode(&arg.to_string())));
    }
    pub fn report_number(arg: &str) -> Self {
        return QueryParams::ReportNumber(format!("rn:\"{}\"", encode(arg)));
    }
    pub fn id(id: &str) -> Self {
        return QueryParams::Id(format!("id:\"{}\"", encode(id)));
    }
    pub fn all(arg: &str) -> Self {
        return QueryParams::All(format!("all:\"{}\"", encode(arg)));
    }
    pub fn to_string(&self) -> String {
        match self {
            QueryParams::Title(arg) => arg.to_string(),
            QueryParams::Author(arg) => arg.to_string(),
            QueryParams::Abstract(arg) => arg.to_string(),
            QueryParams::Comment(arg) => arg.to_string(),
            QueryParams::JournalRef(arg) => arg.to_string(),
            QueryParams::SubjectCategory(arg) => arg.to_string(),
            QueryParams::ReportNumber(arg) => arg.to_string(),
            QueryParams::Id(arg) => arg.to_string(),
            QueryParams::All(arg) => arg.to_string(),
            QueryParams::And(arg) => arg.to_string(),
            QueryParams::Or(arg) => arg.to_string(),
            QueryParams::AndNot(arg) => arg.to_string(),
            QueryParams::Group(arg) => arg.to_string(),
            QueryParams::SubmittedDate(from, to) => {
                format!("submittedDate:[{}+TO+{}]", from, to)
            }
        }
    }
    pub fn and(args: Vec<QueryParams>) -> Self {
        let args = args
            .iter()
            .map(|arg| arg.to_string())
            .collect::<Vec<String>>();
        let query = args.join("+AND+");
        return QueryParams::And(query);
    }
    pub fn or(args: Vec<QueryParams>) -> Self {
        let args = args
            .iter()
            .map(|arg| arg.to_string())
            .collect::<Vec<String>>();
        let query = args.join("+OR+");
        return QueryParams::Or(query);
    }
    pub fn and_not(args: Vec<QueryParams>) -> Self {
        let args = args
            .iter()
            .map(|arg| arg.to_string())
            .collect::<Vec<String>>();
        let query = args.join("+ANDNOT+");
        return QueryParams::Or(query);
    }
    pub fn group(args: Vec<QueryParams>) -> Self {
        let mut args = args
            .iter()
            .map(|arg| arg.to_string())
            .collect::<Vec<String>>();
        args.insert(0, String::from("%28"));
        args.push(String::from("%29"));
        let query = args.join("");
        return QueryParams::Group(query);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paper {
    pub id: String,
    pub title: String,
    pub authors: Vec<String>,
    #[serde(rename = "abstract")]
    pub abstract_text: String,
    pub published: String,
    pub updated: String,
    pub doi: String,
    pub comment: Vec<String>,
    pub journal_ref: String,
    pub pdf_url: String,
    pub primary_category: String,
    pub categories: Vec<String>,
}

impl Paper {
    pub fn default() -> Self {
        return Paper {
            id: "".to_string(),
            title: "".to_string(),
            authors: Vec::new(),
            abstract_text: "".to_string(),
            published: "".to_string(),
            updated: "".to_string(),
            doi: "".to_string(),
            comment: Vec::new(),
            journal_ref: "".to_string(),
            pdf_url: "".to_string(),
            primary_category: "".to_string(),
            categories: Vec::new(),
        };
    }

    pub fn published2utc(&self) -> DateTime<Utc> {
        return DateTime::parse_from_rfc3339(&self.published)
            .unwrap()
            .with_timezone(&Utc);
    }

    pub fn updated2utc(&self) -> DateTime<Utc> {
        return DateTime::parse_from_rfc3339(&self.updated)
            .unwrap()
            .with_timezone(&Utc);
    }
}

#[derive(Clone, Debug, Default)]
pub struct ArXiv {
    pub args: QueryParams,
    pub start: Option<u64>,
    pub max_resutls: Option<u64>,
    pub sort_by: Option<SortBy>,
    pub sort_order: Option<SortOrder>,
}

impl ArXiv {
    pub fn from_args(args: QueryParams) -> Self {
        return ArXiv {
            args: args,
            max_resutls: None,
            start: None,
            sort_by: None,
            sort_order: None,
        };
    }

    pub fn start(&mut self, start: u64) -> &mut Self {
        self.start = Some(start);
        return self;
    }
    pub fn max_results(&mut self, max_results: u64) -> &mut Self {
        self.max_resutls = Some(max_results);
        return self;
    }
    pub fn sort_by(&mut self, sort_by: SortBy) -> &mut Self {
        self.sort_by = Some(sort_by);
        return self;
    }
    pub fn sort_order(&mut self, sort_order: SortOrder) -> &mut Self {
        self.sort_order = Some(sort_order);
        return self;
    }

    fn parse_xml(&self, xml: String) -> Vec<Paper> {
        let mut reader = Reader::from_str(&xml);
        let mut buf = Vec::new();
        let mut in_entry = false;
        let mut in_id = false;
        let mut in_title = false;
        let mut in_author = false;
        let mut in_name = false;
        let mut in_abstract = false;
        let mut in_published = false;
        let mut in_updated = false;
        let mut in_comment = false;
        let mut in_journal_ref = false;

        let mut responses: Vec<Paper> = Vec::new();
        let mut res = Paper::default();
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    if e.name().as_ref() == b"entry" {
                        in_entry = true;
                        res = Paper::default();
                    } else if e.name().as_ref() == b"id" {
                        in_id = true;
                    } else if e.name().as_ref() == b"title" {
                        in_title = true;
                    } else if e.name().as_ref() == b"author" {
                        in_author = true;
                    } else if e.name().as_ref() == b"name" {
                        if in_author {
                            in_name = true;
                        }
                    } else if e.name().as_ref() == b"summary" {
                        in_abstract = true;
                    } else if e.name().as_ref() == b"published" {
                        in_published = true;
                    } else if e.name().as_ref() == b"updated" {
                        in_updated = true;
                    } else if e.name().as_ref() == b"arxiv:comment" {
                        in_comment = true;
                    } else if e.name().as_ref() == b"arxiv:journal_ref" {
                        in_journal_ref = true;
                    } else if e.name().as_ref() == b"link" && in_entry {
                        let mut is_pdf = false;
                        let mut is_doi = false;
                        e.attributes().for_each(|attr| {
                            if let Ok(attr) = attr {
                                if attr.key.as_ref() == b"title" && attr.value.as_ref() == b"pdf" {
                                    is_pdf = true;
                                } else if attr.key.as_ref() == b"title"
                                    && attr.value.as_ref() == b"doi"
                                {
                                    is_doi = true;
                                }
                            }
                        });
                        e.attributes().for_each(|attr| {
                            if let Ok(attr) = attr {
                                if attr.key.as_ref() == b"href" {
                                    if is_pdf {
                                        res.pdf_url = String::from_utf8_lossy(attr.value.as_ref())
                                            .to_string();
                                    } else if is_doi {
                                        res.doi = String::from_utf8_lossy(attr.value.as_ref())
                                            .to_string();
                                    }
                                }
                            }
                        });
                    } else if e.name().as_ref() == b"arxiv:primary_category" {
                        e.attributes().for_each(|attr| {
                            if let Ok(attr) = attr {
                                if attr.key.as_ref() == b"term" {
                                    res.primary_category =
                                        String::from_utf8_lossy(attr.value.as_ref()).to_string();
                                }
                            }
                        });
                    } else if e.name().as_ref() == b"category" {
                        if let Some(attr) = e
                            .attributes()
                            .find(|attr| attr.as_ref().unwrap().key.as_ref() == b"term")
                        {
                            res.categories.push(
                                String::from_utf8_lossy(attr.unwrap().value.as_ref()).to_string(),
                            );
                        }
                    } else if e.name().as_ref() == b"category" {
                        if let Some(attr) = e
                            .attributes()
                            .find(|attr| attr.as_ref().unwrap().key.as_ref() == b"term")
                        {
                            res.categories.push(
                                String::from_utf8_lossy(attr.unwrap().value.as_ref()).to_string(),
                            );
                        }
                    }
                }
                Ok(Event::End(ref e)) => {
                    if e.name().as_ref() == b"entry" {
                        in_entry = false;
                        responses.push(res.clone());
                        res = Paper::default();
                    } else if e.name().as_ref() == b"id" {
                        in_id = false;
                    } else if e.name().as_ref() == b"title" {
                        in_title = false;
                    } else if e.name().as_ref() == b"author" {
                        in_author = false;
                    } else if e.name().as_ref() == b"name" {
                        if in_author {
                            in_name = false;
                        }
                    } else if e.name().as_ref() == b"summary" {
                        in_abstract = false;
                    } else if e.name().as_ref() == b"published" {
                        in_published = false;
                    } else if e.name().as_ref() == b"updated" {
                        in_updated = false;
                    } else if e.name().as_ref() == b"arxiv:comment" {
                        in_comment = false;
                    } else if e.name().as_ref() == b"arxiv:journal_ref" {
                        in_journal_ref = true;
                    }
                }
                Ok(Event::Text(e)) => {
                    if in_entry {
                        if in_id {
                            res.id = e.unescape().unwrap().to_string();
                        } else if in_title {
                            res.title = e.unescape().unwrap().to_string();
                        } else if in_author && in_name {
                            res.authors.push(e.unescape().unwrap().to_string());
                        } else if in_abstract {
                            res.abstract_text =
                                e.unescape().unwrap().to_string().trim().replace("\n", "");
                        } else if in_published {
                            res.published = e.unescape().unwrap().to_string();
                        } else if in_updated {
                            res.updated = e.unescape().unwrap().to_string();
                        } else if in_comment {
                            res.comment.push(e.unescape().unwrap().to_string());
                        } else if in_journal_ref {
                            res.journal_ref = e.unescape().unwrap().to_string();
                        }
                    }
                }
                Ok(Event::Empty(ref e)) => {
                    if e.name().as_ref() == b"link" && in_entry {
                        let mut is_pdf = false;
                        let mut is_doi = false;
                        e.attributes().for_each(|attr| {
                            if let Ok(attr) = attr {
                                if attr.key.as_ref() == b"title" && attr.value.as_ref() == b"pdf" {
                                    is_pdf = true;
                                } else if attr.key.as_ref() == b"title"
                                    && attr.value.as_ref() == b"doi"
                                {
                                    is_doi = true;
                                }
                            }
                        });
                        e.attributes().for_each(|attr| {
                            if let Ok(attr) = attr {
                                if attr.key.as_ref() == b"href" {
                                    if is_pdf {
                                        res.pdf_url = String::from_utf8_lossy(attr.value.as_ref())
                                            .to_string();
                                    } else if is_doi {
                                        res.doi = String::from_utf8_lossy(attr.value.as_ref())
                                            .to_string();
                                    }
                                }
                            }
                        });
                    } else if e.name().as_ref() == b"arxiv:primary_category" && in_entry {
                        e.attributes().for_each(|attr| {
                            if let Ok(attr) = attr {
                                if attr.key.as_ref() == b"term" {
                                    res.primary_category =
                                        String::from_utf8_lossy(attr.value.as_ref()).to_string();
                                }
                            }
                        });
                    } else if e.name().as_ref() == b"category" && in_entry {
                        if let Some(attr) = e
                            .attributes()
                            .find(|attr| attr.as_ref().unwrap().key.as_ref() == b"term")
                        {
                            res.categories.push(
                                String::from_utf8_lossy(attr.unwrap().value.as_ref()).to_string(),
                            );
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => panic!("Error at position {}: {:?}", reader.buffer_position(), e),
                _ => (),
            }
            buf.clear();
        }
        return responses;
    }

    fn build_query(&self) -> String {
        let mut query = self.args.to_string();
        query = query.replace("%20", "+");
        if let Some(start) = &self.start {
            query.push_str(&format!("&start={}", start));
        }
        if let Some(max_resutls) = &self.max_resutls {
            query.push_str(&format!("&max_results={}", max_resutls));
        }
        if let Some(sort_by) = &self.sort_by {
            query.push_str(&format!("&sortBy={}", sort_by.to_string()));
        }
        if let Some(sort_order) = &self.sort_order {
            query.push_str(&format!("&sortOrder={}", sort_order.to_string()));
        }

        return format!("http://export.arxiv.org/api/query?search_query={}", query);
    }

    pub async fn query(&mut self) -> Vec<Paper> {
        let url = self.build_query();
        let body = request::get(&url).await.unwrap().text().await.unwrap();
        let responses = self.parse_xml(body);
        return responses;
    }
}

#[cfg(test)]
mod tests;
