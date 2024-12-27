use percent_encoding::utf8_percent_encode;
use percent_encoding::NON_ALPHANUMERIC as NON_ALNUM;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use reqwest as request;
use serde::{Deserialize, Serialize};

fn encode(s: &str) -> String {
    utf8_percent_encode(s, NON_ALNUM)
        .to_string()
        .replace("%20", "+")
}

pub enum ArXivCategory {
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

impl ArXivCategory {
    pub fn to_string(&self) -> String {
        match self {
            ArXivCategory::CsAi => String::from("cs.AI"),
            ArXivCategory::CsCl => String::from("cs.CL"),
            ArXivCategory::CsLg => String::from("cs.LG"),
            ArXivCategory::CsGt => String::from("cs.GT"),
            ArXivCategory::CsCv => String::from("cs.CV"),
            ArXivCategory::CsCr => String::from("cs.CR"),
            ArXivCategory::CsCc => String::from("cs.CC"),
            ArXivCategory::CsCe => String::from("cs.CE"),
            ArXivCategory::CsCy => String::from("cs.CY"),
            ArXivCategory::CsDs => String::from("cs.DS"),
            ArXivCategory::CsDm => String::from("cs.DM"),
            ArXivCategory::CsDc => String::from("cs.DC"),
            ArXivCategory::CsEt => String::from("cs.ET"),
            ArXivCategory::CsFl => String::from("cs.FL"),
            ArXivCategory::CsGl => String::from("cs.GL"),
            ArXivCategory::CsGr => String::from("cs.GR"),
            ArXivCategory::CsAr => String::from("cs.AR"),
            ArXivCategory::CsHc => String::from("cs.HC"),
            ArXivCategory::CsIr => String::from("cs.IR"),
        }
    }
}

#[derive(Clone, Debug)]
pub enum ArXivArgs {
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
}

impl Default for ArXivArgs {
    fn default() -> Self {
        return ArXivArgs::title("default");
    }
}

impl ArXivArgs {
    pub fn title(arg: &str) -> Self {
        return ArXivArgs::Title(encode(&format!("ti:\"{}\"", arg)));
    }
    pub fn author(arg: &str) -> Self {
        return ArXivArgs::Author(encode(&format!("au:\"{}\"", arg)));
    }
    pub fn abstract_text(arg: &str) -> Self {
        return ArXivArgs::Abstract(encode(&format!("abs:\"{}\"", arg)));
    }
    pub fn comment(arg: &str) -> Self {
        return ArXivArgs::Comment(encode(&format!("co:\"{}\"", arg)));
    }
    pub fn journal_ref(arg: &str) -> Self {
        return ArXivArgs::JournalRef(encode(&format!("jr:\"{}\"", arg)));
    }
    pub fn subject_category(arg: ArXivCategory) -> Self {
        return ArXivArgs::SubjectCategory(encode(&format!("cat:\"{}\"", arg.to_string())));
    }
    pub fn report_number(arg: &str) -> Self {
        return ArXivArgs::ReportNumber(encode(&format!("rn:\"{}\"", arg)));
    }
    pub fn id(id: &str) -> Self {
        return ArXivArgs::Id(encode(&format!("id:\"{}\"", id)));
    }
    pub fn all(arg: &str) -> Self {
        return ArXivArgs::All(encode(&format!("all:\"{}\"", arg)));
    }
    pub fn to_string(&self) -> String {
        match self {
            ArXivArgs::Title(arg) => arg.to_string(),
            ArXivArgs::Author(arg) => arg.to_string(),
            ArXivArgs::Abstract(arg) => arg.to_string(),
            ArXivArgs::Comment(arg) => arg.to_string(),
            ArXivArgs::JournalRef(arg) => arg.to_string(),
            ArXivArgs::SubjectCategory(arg) => arg.to_string(),
            ArXivArgs::ReportNumber(arg) => arg.to_string(),
            ArXivArgs::Id(arg) => arg.to_string(),
            ArXivArgs::All(arg) => arg.to_string(),
            ArXivArgs::And(arg) => arg.to_string(),
            ArXivArgs::Or(arg) => arg.to_string(),
            ArXivArgs::AndNot(arg) => arg.to_string(),
            ArXivArgs::Group(arg) => arg.to_string(),
        }
    }
    pub fn and(args: Vec<ArXivArgs>) -> Self {
        let args = args
            .iter()
            .map(|arg| arg.to_string())
            .collect::<Vec<String>>();
        let query = args.join(&encode(" AND "));
        return ArXivArgs::And(query);
    }
    pub fn or(args: Vec<ArXivArgs>) -> Self {
        let args = args
            .iter()
            .map(|arg| arg.to_string())
            .collect::<Vec<String>>();
        let query = args.join(&encode(" OR "));
        return ArXivArgs::Or(query);
    }
    pub fn and_not(args: Vec<ArXivArgs>) -> Self {
        let args = args
            .iter()
            .map(|arg| arg.to_string())
            .collect::<Vec<String>>();
        let query = args.join(&encode(" ANDNOT "));
        return ArXivArgs::Or(query);
    }
    pub fn group(args: Vec<ArXivArgs>) -> Self {
        let mut args = args
            .iter()
            .map(|arg| arg.to_string())
            .collect::<Vec<String>>();
        args.insert(0, encode("("));
        args.push(encode(")"));
        let query = args.join("");
        return ArXivArgs::Group(query);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArXivResponse {
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

impl ArXivResponse {
    pub fn default() -> Self {
        return ArXivResponse {
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
}

#[derive(Clone, Debug, Default)]
pub struct ArXiv {
    pub url: String,
    pub base_url: String,
    pub args: ArXivArgs,
    pub submitted_date: String,
}

impl ArXiv {
    pub fn from_args(args: ArXivArgs) -> Self {
        return ArXiv {
            base_url: "http://export.arxiv.org/api/query?search_query=".to_string(),
            url: "".to_string(),
            args: args,
            submitted_date: "".to_string(),
        };
    }

    pub fn build_query(&mut self, args: ArXivArgs) {
        self.args = args;
    }

    pub fn submitted_date(&mut self, from: &str, to: &str) -> &mut Self {
        self.submitted_date = format!("&submittedDate:[{}+TO+{}]", from, to);
        return self;
    }

    fn parse_xml(&self, xml: String) -> Vec<ArXivResponse> {
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

        let mut responses: Vec<ArXivResponse> = Vec::new();
        let mut res = ArXivResponse::default();
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    if e.name().as_ref() == b"entry" {
                        in_entry = true;
                        res = ArXivResponse::default();
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
                        res = ArXivResponse::default();
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

    pub async fn query(&mut self) -> Vec<ArXivResponse> {
        let query = self.args.to_string();
        self.url = format!(
            "{}{}&sortBy=lastUpdatedDate&sortOrder=descending",
            self.base_url, query,
        );
        self.url = self.url.replace("%20", "+");

        let body = request::get(&self.url).await.unwrap().text().await.unwrap();
        let responses = self.parse_xml(body);
        return responses;
    }
}

#[cfg(test)]
mod tests;
