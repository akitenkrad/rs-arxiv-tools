//! arXiv subject categories.
//!
//! [`Category`] covers the complete arXiv taxonomy (as published at
//! <https://arxiv.org/category_taxonomy>). A code that arXiv adds in the
//! future — or an alias such as the legacy `cmp-lg` — still parses, landing in
//! [`Category::Other`], so no category is ever unreachable.
//!
//! Parsing checks the shape of the code, so a typo or a string carrying query
//! syntax is an error rather than a silently unmatchable `Other`.
//!
//! ```
//! use arxiv_tools::Category;
//!
//! assert_eq!(Category::CsLg.as_str(), "cs.LG");
//! assert_eq!("stat.ML".parse::<Category>().unwrap(), Category::StatMl);
//!
//! // Unknown but well-formed: kept verbatim.
//! assert_eq!(
//!     "cs.FUTURE".parse::<Category>().unwrap(),
//!     Category::other("cs.FUTURE").unwrap()
//! );
//! assert_eq!("cs.FUTURE".parse::<Category>().unwrap().as_str(), "cs.FUTURE");
//!
//! // Malformed: rejected.
//! assert!(r#"cs.AI" OR cat:"cs.LG"#.parse::<Category>().is_err());
//! ```

use std::fmt;
use std::str::FromStr;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{Error, Result};

/// Longest category code this crate will accept, comfortably above the
/// longest real one (`cond-mat.quant-gas`, 18 characters).
const MAX_CODE_LEN: usize = 40;

/// An arXiv subject category, such as `cs.LG` or `math-ph`.
///
/// Use [`Category::as_str`] to get the wire representation and
/// [`Category::all`] to enumerate every known category.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Category {
    // ---- Computer Science ----
    /// `cs.AI` — Artificial Intelligence
    CsAi,
    /// `cs.AR` — Hardware Architecture
    CsAr,
    /// `cs.CC` — Computational Complexity
    CsCc,
    /// `cs.CE` — Computational Engineering, Finance, and Science
    CsCe,
    /// `cs.CG` — Computational Geometry
    CsCg,
    /// `cs.CL` — Computation and Language
    CsCl,
    /// `cs.CR` — Cryptography and Security
    CsCr,
    /// `cs.CV` — Computer Vision and Pattern Recognition
    CsCv,
    /// `cs.CY` — Computers and Society
    CsCy,
    /// `cs.DB` — Databases
    CsDb,
    /// `cs.DC` — Distributed, Parallel, and Cluster Computing
    CsDc,
    /// `cs.DL` — Digital Libraries
    CsDl,
    /// `cs.DM` — Discrete Mathematics
    CsDm,
    /// `cs.DS` — Data Structures and Algorithms
    CsDs,
    /// `cs.ET` — Emerging Technologies
    CsEt,
    /// `cs.FL` — Formal Languages and Automata Theory
    CsFl,
    /// `cs.GL` — General Literature
    CsGl,
    /// `cs.GR` — Graphics
    CsGr,
    /// `cs.GT` — Computer Science and Game Theory
    CsGt,
    /// `cs.HC` — Human-Computer Interaction
    CsHc,
    /// `cs.IR` — Information Retrieval
    CsIr,
    /// `cs.IT` — Information Theory
    CsIt,
    /// `cs.LG` — Machine Learning
    CsLg,
    /// `cs.LO` — Logic in Computer Science
    CsLo,
    /// `cs.MA` — Multiagent Systems
    CsMa,
    /// `cs.MM` — Multimedia
    CsMm,
    /// `cs.MS` — Mathematical Software
    CsMs,
    /// `cs.NA` — Numerical Analysis
    CsNa,
    /// `cs.NE` — Neural and Evolutionary Computing
    CsNe,
    /// `cs.NI` — Networking and Internet Architecture
    CsNi,
    /// `cs.OH` — Other Computer Science
    CsOh,
    /// `cs.OS` — Operating Systems
    CsOs,
    /// `cs.PF` — Performance
    CsPf,
    /// `cs.PL` — Programming Languages
    CsPl,
    /// `cs.RO` — Robotics
    CsRo,
    /// `cs.SC` — Symbolic Computation
    CsSc,
    /// `cs.SD` — Sound
    CsSd,
    /// `cs.SE` — Software Engineering
    CsSe,
    /// `cs.SI` — Social and Information Networks
    CsSi,
    /// `cs.SY` — Systems and Control
    CsSy,

    // ---- Economics ----
    /// `econ.EM` — Econometrics
    EconEm,
    /// `econ.GN` — General Economics
    EconGn,
    /// `econ.TH` — Theoretical Economics
    EconTh,

    // ---- Electrical Engineering and Systems Science ----
    /// `eess.AS` — Audio and Speech Processing
    EessAs,
    /// `eess.IV` — Image and Video Processing
    EessIv,
    /// `eess.SP` — Signal Processing
    EessSp,
    /// `eess.SY` — Systems and Control
    EessSy,

    // ---- Mathematics ----
    /// `math.AC` — Commutative Algebra
    MathAc,
    /// `math.AG` — Algebraic Geometry
    MathAg,
    /// `math.AP` — Analysis of PDEs
    MathAp,
    /// `math.AT` — Algebraic Topology
    MathAt,
    /// `math.CA` — Classical Analysis and ODEs
    MathCa,
    /// `math.CO` — Combinatorics
    MathCo,
    /// `math.CT` — Category Theory
    MathCt,
    /// `math.CV` — Complex Variables
    MathCv,
    /// `math.DG` — Differential Geometry
    MathDg,
    /// `math.DS` — Dynamical Systems
    MathDs,
    /// `math.FA` — Functional Analysis
    MathFa,
    /// `math.GM` — General Mathematics
    MathGm,
    /// `math.GN` — General Topology
    MathGn,
    /// `math.GR` — Group Theory
    MathGr,
    /// `math.GT` — Geometric Topology
    MathGt,
    /// `math.HO` — History and Overview
    MathHo,
    /// `math.IT` — Information Theory
    MathIt,
    /// `math.KT` — K-Theory and Homology
    MathKt,
    /// `math.LO` — Logic
    MathLo,
    /// `math.MG` — Metric Geometry
    MathMg,
    /// `math.MP` — Mathematical Physics
    MathMp,
    /// `math.NA` — Numerical Analysis
    MathNa,
    /// `math.NT` — Number Theory
    MathNt,
    /// `math.OA` — Operator Algebras
    MathOa,
    /// `math.OC` — Optimization and Control
    MathOc,
    /// `math.PR` — Probability
    MathPr,
    /// `math.QA` — Quantum Algebra
    MathQa,
    /// `math.RA` — Rings and Algebras
    MathRa,
    /// `math.RT` — Representation Theory
    MathRt,
    /// `math.SG` — Symplectic Geometry
    MathSg,
    /// `math.SP` — Spectral Theory
    MathSp,
    /// `math.ST` — Statistics Theory
    MathSt,

    // ---- Physics: Astrophysics ----
    /// `astro-ph.CO` — Cosmology and Nongalactic Astrophysics
    AstroPhCo,
    /// `astro-ph.EP` — Earth and Planetary Astrophysics
    AstroPhEp,
    /// `astro-ph.GA` — Astrophysics of Galaxies
    AstroPhGa,
    /// `astro-ph.HE` — High Energy Astrophysical Phenomena
    AstroPhHe,
    /// `astro-ph.IM` — Instrumentation and Methods for Astrophysics
    AstroPhIm,
    /// `astro-ph.SR` — Solar and Stellar Astrophysics
    AstroPhSr,

    // ---- Physics: Condensed Matter ----
    /// `cond-mat.dis-nn` — Disordered Systems and Neural Networks
    CondMatDisNn,
    /// `cond-mat.mes-hall` — Mesoscale and Nanoscale Physics
    CondMatMesHall,
    /// `cond-mat.mtrl-sci` — Materials Science
    CondMatMtrlSci,
    /// `cond-mat.other` — Other Condensed Matter
    CondMatOther,
    /// `cond-mat.quant-gas` — Quantum Gases
    CondMatQuantGas,
    /// `cond-mat.soft` — Soft Condensed Matter
    CondMatSoft,
    /// `cond-mat.stat-mech` — Statistical Mechanics
    CondMatStatMech,
    /// `cond-mat.str-el` — Strongly Correlated Electrons
    CondMatStrEl,
    /// `cond-mat.supr-con` — Superconductivity
    CondMatSuprCon,

    // ---- Physics ----
    /// `gr-qc` — General Relativity and Quantum Cosmology
    GrQc,
    /// `hep-ex` — High Energy Physics - Experiment
    HepEx,
    /// `hep-lat` — High Energy Physics - Lattice
    HepLat,
    /// `hep-ph` — High Energy Physics - Phenomenology
    HepPh,
    /// `hep-th` — High Energy Physics - Theory
    HepTh,
    /// `math-ph` — Mathematical Physics
    MathPh,
    /// `nucl-ex` — Nuclear Experiment
    NuclEx,
    /// `nucl-th` — Nuclear Theory
    NuclTh,
    /// `physics.acc-ph` — Accelerator Physics
    PhysicsAccPh,
    /// `physics.ao-ph` — Atmospheric and Oceanic Physics
    PhysicsAoPh,
    /// `physics.app-ph` — Applied Physics
    PhysicsAppPh,
    /// `physics.atm-clus` — Atomic and Molecular Clusters
    PhysicsAtmClus,
    /// `physics.atom-ph` — Atomic Physics
    PhysicsAtomPh,
    /// `physics.bio-ph` — Biological Physics
    PhysicsBioPh,
    /// `physics.chem-ph` — Chemical Physics
    PhysicsChemPh,
    /// `physics.class-ph` — Classical Physics
    PhysicsClassPh,
    /// `physics.comp-ph` — Computational Physics
    PhysicsCompPh,
    /// `physics.data-an` — Data Analysis, Statistics and Probability
    PhysicsDataAn,
    /// `physics.ed-ph` — Physics Education
    PhysicsEdPh,
    /// `physics.flu-dyn` — Fluid Dynamics
    PhysicsFluDyn,
    /// `physics.gen-ph` — General Physics
    PhysicsGenPh,
    /// `physics.geo-ph` — Geophysics
    PhysicsGeoPh,
    /// `physics.hist-ph` — History and Philosophy of Physics
    PhysicsHistPh,
    /// `physics.ins-det` — Instrumentation and Detectors
    PhysicsInsDet,
    /// `physics.med-ph` — Medical Physics
    PhysicsMedPh,
    /// `physics.optics` — Optics
    PhysicsOptics,
    /// `physics.plasm-ph` — Plasma Physics
    PhysicsPlasmPh,
    /// `physics.pop-ph` — Popular Physics
    PhysicsPopPh,
    /// `physics.soc-ph` — Physics and Society
    PhysicsSocPh,
    /// `physics.space-ph` — Space Physics
    PhysicsSpacePh,
    /// `quant-ph` — Quantum Physics
    QuantPh,

    // ---- Physics: Nonlinear Sciences ----
    /// `nlin.AO` — Adaptation and Self-Organizing Systems
    NlinAo,
    /// `nlin.CD` — Chaotic Dynamics
    NlinCd,
    /// `nlin.CG` — Cellular Automata and Lattice Gases
    NlinCg,
    /// `nlin.PS` — Pattern Formation and Solitons
    NlinPs,
    /// `nlin.SI` — Exactly Solvable and Integrable Systems
    NlinSi,

    // ---- Quantitative Biology ----
    /// `q-bio.BM` — Biomolecules
    QBioBm,
    /// `q-bio.CB` — Cell Behavior
    QBioCb,
    /// `q-bio.GN` — Genomics
    QBioGn,
    /// `q-bio.MN` — Molecular Networks
    QBioMn,
    /// `q-bio.NC` — Neurons and Cognition
    QBioNc,
    /// `q-bio.OT` — Other Quantitative Biology
    QBioOt,
    /// `q-bio.PE` — Populations and Evolution
    QBioPe,
    /// `q-bio.QM` — Quantitative Methods
    QBioQm,
    /// `q-bio.SC` — Subcellular Processes
    QBioSc,
    /// `q-bio.TO` — Tissues and Organs
    QBioTo,

    // ---- Quantitative Finance ----
    /// `q-fin.CP` — Computational Finance
    QFinCp,
    /// `q-fin.EC` — Economics
    QFinEc,
    /// `q-fin.GN` — General Finance
    QFinGn,
    /// `q-fin.MF` — Mathematical Finance
    QFinMf,
    /// `q-fin.PM` — Portfolio Management
    QFinPm,
    /// `q-fin.PR` — Pricing of Securities
    QFinPr,
    /// `q-fin.RM` — Risk Management
    QFinRm,
    /// `q-fin.ST` — Statistical Finance
    QFinSt,
    /// `q-fin.TR` — Trading and Market Microstructure
    QFinTr,

    // ---- Statistics ----
    /// `stat.AP` — Applications
    StatAp,
    /// `stat.CO` — Computation
    StatCo,
    /// `stat.ME` — Methodology
    StatMe,
    /// `stat.ML` — Machine Learning
    StatMl,
    /// `stat.OT` — Other Statistics
    StatOt,
    /// `stat.TH` — Statistics Theory
    StatTh,

    /// A category that is not one of the known variants, stored verbatim.
    ///
    /// Produced by [`Category::from_str`] for unrecognised codes, which lets
    /// you query categories this crate does not know about yet. The payload
    /// is a [`CategoryCode`], so this variant cannot be built with a string
    /// that is not a category code.
    Other(CategoryCode),
}

impl Category {
    /// The arXiv wire representation of this category, e.g. `"cs.LG"`.
    pub fn as_str(&self) -> &str {
        match self {
            Category::CsAi => "cs.AI",
            Category::CsAr => "cs.AR",
            Category::CsCc => "cs.CC",
            Category::CsCe => "cs.CE",
            Category::CsCg => "cs.CG",
            Category::CsCl => "cs.CL",
            Category::CsCr => "cs.CR",
            Category::CsCv => "cs.CV",
            Category::CsCy => "cs.CY",
            Category::CsDb => "cs.DB",
            Category::CsDc => "cs.DC",
            Category::CsDl => "cs.DL",
            Category::CsDm => "cs.DM",
            Category::CsDs => "cs.DS",
            Category::CsEt => "cs.ET",
            Category::CsFl => "cs.FL",
            Category::CsGl => "cs.GL",
            Category::CsGr => "cs.GR",
            Category::CsGt => "cs.GT",
            Category::CsHc => "cs.HC",
            Category::CsIr => "cs.IR",
            Category::CsIt => "cs.IT",
            Category::CsLg => "cs.LG",
            Category::CsLo => "cs.LO",
            Category::CsMa => "cs.MA",
            Category::CsMm => "cs.MM",
            Category::CsMs => "cs.MS",
            Category::CsNa => "cs.NA",
            Category::CsNe => "cs.NE",
            Category::CsNi => "cs.NI",
            Category::CsOh => "cs.OH",
            Category::CsOs => "cs.OS",
            Category::CsPf => "cs.PF",
            Category::CsPl => "cs.PL",
            Category::CsRo => "cs.RO",
            Category::CsSc => "cs.SC",
            Category::CsSd => "cs.SD",
            Category::CsSe => "cs.SE",
            Category::CsSi => "cs.SI",
            Category::CsSy => "cs.SY",
            Category::EconEm => "econ.EM",
            Category::EconGn => "econ.GN",
            Category::EconTh => "econ.TH",
            Category::EessAs => "eess.AS",
            Category::EessIv => "eess.IV",
            Category::EessSp => "eess.SP",
            Category::EessSy => "eess.SY",
            Category::MathAc => "math.AC",
            Category::MathAg => "math.AG",
            Category::MathAp => "math.AP",
            Category::MathAt => "math.AT",
            Category::MathCa => "math.CA",
            Category::MathCo => "math.CO",
            Category::MathCt => "math.CT",
            Category::MathCv => "math.CV",
            Category::MathDg => "math.DG",
            Category::MathDs => "math.DS",
            Category::MathFa => "math.FA",
            Category::MathGm => "math.GM",
            Category::MathGn => "math.GN",
            Category::MathGr => "math.GR",
            Category::MathGt => "math.GT",
            Category::MathHo => "math.HO",
            Category::MathIt => "math.IT",
            Category::MathKt => "math.KT",
            Category::MathLo => "math.LO",
            Category::MathMg => "math.MG",
            Category::MathMp => "math.MP",
            Category::MathNa => "math.NA",
            Category::MathNt => "math.NT",
            Category::MathOa => "math.OA",
            Category::MathOc => "math.OC",
            Category::MathPr => "math.PR",
            Category::MathQa => "math.QA",
            Category::MathRa => "math.RA",
            Category::MathRt => "math.RT",
            Category::MathSg => "math.SG",
            Category::MathSp => "math.SP",
            Category::MathSt => "math.ST",
            Category::AstroPhCo => "astro-ph.CO",
            Category::AstroPhEp => "astro-ph.EP",
            Category::AstroPhGa => "astro-ph.GA",
            Category::AstroPhHe => "astro-ph.HE",
            Category::AstroPhIm => "astro-ph.IM",
            Category::AstroPhSr => "astro-ph.SR",
            Category::CondMatDisNn => "cond-mat.dis-nn",
            Category::CondMatMesHall => "cond-mat.mes-hall",
            Category::CondMatMtrlSci => "cond-mat.mtrl-sci",
            Category::CondMatOther => "cond-mat.other",
            Category::CondMatQuantGas => "cond-mat.quant-gas",
            Category::CondMatSoft => "cond-mat.soft",
            Category::CondMatStatMech => "cond-mat.stat-mech",
            Category::CondMatStrEl => "cond-mat.str-el",
            Category::CondMatSuprCon => "cond-mat.supr-con",
            Category::GrQc => "gr-qc",
            Category::HepEx => "hep-ex",
            Category::HepLat => "hep-lat",
            Category::HepPh => "hep-ph",
            Category::HepTh => "hep-th",
            Category::MathPh => "math-ph",
            Category::NuclEx => "nucl-ex",
            Category::NuclTh => "nucl-th",
            Category::PhysicsAccPh => "physics.acc-ph",
            Category::PhysicsAoPh => "physics.ao-ph",
            Category::PhysicsAppPh => "physics.app-ph",
            Category::PhysicsAtmClus => "physics.atm-clus",
            Category::PhysicsAtomPh => "physics.atom-ph",
            Category::PhysicsBioPh => "physics.bio-ph",
            Category::PhysicsChemPh => "physics.chem-ph",
            Category::PhysicsClassPh => "physics.class-ph",
            Category::PhysicsCompPh => "physics.comp-ph",
            Category::PhysicsDataAn => "physics.data-an",
            Category::PhysicsEdPh => "physics.ed-ph",
            Category::PhysicsFluDyn => "physics.flu-dyn",
            Category::PhysicsGenPh => "physics.gen-ph",
            Category::PhysicsGeoPh => "physics.geo-ph",
            Category::PhysicsHistPh => "physics.hist-ph",
            Category::PhysicsInsDet => "physics.ins-det",
            Category::PhysicsMedPh => "physics.med-ph",
            Category::PhysicsOptics => "physics.optics",
            Category::PhysicsPlasmPh => "physics.plasm-ph",
            Category::PhysicsPopPh => "physics.pop-ph",
            Category::PhysicsSocPh => "physics.soc-ph",
            Category::PhysicsSpacePh => "physics.space-ph",
            Category::QuantPh => "quant-ph",
            Category::NlinAo => "nlin.AO",
            Category::NlinCd => "nlin.CD",
            Category::NlinCg => "nlin.CG",
            Category::NlinPs => "nlin.PS",
            Category::NlinSi => "nlin.SI",
            Category::QBioBm => "q-bio.BM",
            Category::QBioCb => "q-bio.CB",
            Category::QBioGn => "q-bio.GN",
            Category::QBioMn => "q-bio.MN",
            Category::QBioNc => "q-bio.NC",
            Category::QBioOt => "q-bio.OT",
            Category::QBioPe => "q-bio.PE",
            Category::QBioQm => "q-bio.QM",
            Category::QBioSc => "q-bio.SC",
            Category::QBioTo => "q-bio.TO",
            Category::QFinCp => "q-fin.CP",
            Category::QFinEc => "q-fin.EC",
            Category::QFinGn => "q-fin.GN",
            Category::QFinMf => "q-fin.MF",
            Category::QFinPm => "q-fin.PM",
            Category::QFinPr => "q-fin.PR",
            Category::QFinRm => "q-fin.RM",
            Category::QFinSt => "q-fin.ST",
            Category::QFinTr => "q-fin.TR",
            Category::StatAp => "stat.AP",
            Category::StatCo => "stat.CO",
            Category::StatMe => "stat.ME",
            Category::StatMl => "stat.ML",
            Category::StatOt => "stat.OT",
            Category::StatTh => "stat.TH",
            Category::Other(code) => code.as_str(),
        }
    }

    /// Every category in the arXiv taxonomy known to this crate.
    ///
    /// [`Category::Other`] is not included.
    pub fn all() -> &'static [Category] {
        &ALL
    }

    /// Builds a [`Category`] from a code this crate does not know, after
    /// checking that it looks like an arXiv category code.
    ///
    /// Prefer a named variant when one exists; [`Category::from_str`] picks
    /// the variant automatically and falls back to this.
    ///
    /// # Errors
    /// Returns [`Error::InvalidParam`] if `code` is not of the form
    /// `archive` or `archive.SUBJECT`.
    pub fn other(code: &str) -> Result<Self> {
        Ok(Category::Other(CategoryCode::new(code)?))
    }
}

/// An arXiv category code this crate does not have a variant for.
///
/// The inner string is private and checked on the way in, so every
/// `CategoryCode` — and therefore every [`Category`] — is usable in a query.
/// That is what lets `Category` round-trip through serde: anything that can
/// be built can also be parsed back.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CategoryCode(String);

impl CategoryCode {
    /// Checks `code` and wraps it.
    ///
    /// # Errors
    /// Returns [`Error::InvalidParam`] if `code` is not of the form
    /// `archive` or `archive.SUBJECT`.
    pub fn new(code: &str) -> Result<Self> {
        if !is_well_formed_code(code) {
            return Err(Error::InvalidParam(format!(
                "{code:?} is not a valid arXiv category code \
                 (expected `archive` or `archive.SUBJECT`, e.g. \"cs.LG\")"
            )));
        }
        Ok(CategoryCode(code.to_string()))
    }

    /// The code itself.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CategoryCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for CategoryCode {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl FromStr for CategoryCode {
    type Err = Error;

    fn from_str(code: &str) -> Result<Self> {
        CategoryCode::new(code)
    }
}

/// Whether `code` has the shape of an arXiv category code.
///
/// Archives are lowercase and may contain hyphens (`cond-mat`, `math-ph`);
/// the optional subject after the dot may be either case (`cs.LG`,
/// `physics.acc-ph`).
fn is_well_formed_code(code: &str) -> bool {
    fn segment(s: &str, lowercase_first: bool) -> bool {
        let mut chars = s.chars();
        let Some(first) = chars.next() else {
            return false;
        };
        let first_ok = if lowercase_first {
            first.is_ascii_lowercase()
        } else {
            first.is_ascii_alphabetic()
        };
        first_ok && chars.all(|c| c.is_ascii_alphanumeric() || c == '-')
    }

    if code.is_empty() || code.len() > MAX_CODE_LEN {
        return false;
    }
    match code.split_once('.') {
        Some((archive, subject)) => segment(archive, true) && segment(subject, false),
        None => segment(code, true),
    }
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Category {
    type Err = Error;

    /// Parses a category code.
    ///
    /// # Errors
    /// Returns [`Error::InvalidParam`] if the code is not of the form
    /// `archive` or `archive.SUBJECT`. Well-formed codes this crate does not
    /// know become [`Category::Other`].
    fn from_str(s: &str) -> Result<Self> {
        Ok(match s {
            "cs.AI" => Category::CsAi,
            "cs.AR" => Category::CsAr,
            "cs.CC" => Category::CsCc,
            "cs.CE" => Category::CsCe,
            "cs.CG" => Category::CsCg,
            "cs.CL" => Category::CsCl,
            "cs.CR" => Category::CsCr,
            "cs.CV" => Category::CsCv,
            "cs.CY" => Category::CsCy,
            "cs.DB" => Category::CsDb,
            "cs.DC" => Category::CsDc,
            "cs.DL" => Category::CsDl,
            "cs.DM" => Category::CsDm,
            "cs.DS" => Category::CsDs,
            "cs.ET" => Category::CsEt,
            "cs.FL" => Category::CsFl,
            "cs.GL" => Category::CsGl,
            "cs.GR" => Category::CsGr,
            "cs.GT" => Category::CsGt,
            "cs.HC" => Category::CsHc,
            "cs.IR" => Category::CsIr,
            "cs.IT" => Category::CsIt,
            "cs.LG" => Category::CsLg,
            "cs.LO" => Category::CsLo,
            "cs.MA" => Category::CsMa,
            "cs.MM" => Category::CsMm,
            "cs.MS" => Category::CsMs,
            "cs.NA" => Category::CsNa,
            "cs.NE" => Category::CsNe,
            "cs.NI" => Category::CsNi,
            "cs.OH" => Category::CsOh,
            "cs.OS" => Category::CsOs,
            "cs.PF" => Category::CsPf,
            "cs.PL" => Category::CsPl,
            "cs.RO" => Category::CsRo,
            "cs.SC" => Category::CsSc,
            "cs.SD" => Category::CsSd,
            "cs.SE" => Category::CsSe,
            "cs.SI" => Category::CsSi,
            "cs.SY" => Category::CsSy,
            "econ.EM" => Category::EconEm,
            "econ.GN" => Category::EconGn,
            "econ.TH" => Category::EconTh,
            "eess.AS" => Category::EessAs,
            "eess.IV" => Category::EessIv,
            "eess.SP" => Category::EessSp,
            "eess.SY" => Category::EessSy,
            "math.AC" => Category::MathAc,
            "math.AG" => Category::MathAg,
            "math.AP" => Category::MathAp,
            "math.AT" => Category::MathAt,
            "math.CA" => Category::MathCa,
            "math.CO" => Category::MathCo,
            "math.CT" => Category::MathCt,
            "math.CV" => Category::MathCv,
            "math.DG" => Category::MathDg,
            "math.DS" => Category::MathDs,
            "math.FA" => Category::MathFa,
            "math.GM" => Category::MathGm,
            "math.GN" => Category::MathGn,
            "math.GR" => Category::MathGr,
            "math.GT" => Category::MathGt,
            "math.HO" => Category::MathHo,
            "math.IT" => Category::MathIt,
            "math.KT" => Category::MathKt,
            "math.LO" => Category::MathLo,
            "math.MG" => Category::MathMg,
            "math.MP" => Category::MathMp,
            "math.NA" => Category::MathNa,
            "math.NT" => Category::MathNt,
            "math.OA" => Category::MathOa,
            "math.OC" => Category::MathOc,
            "math.PR" => Category::MathPr,
            "math.QA" => Category::MathQa,
            "math.RA" => Category::MathRa,
            "math.RT" => Category::MathRt,
            "math.SG" => Category::MathSg,
            "math.SP" => Category::MathSp,
            "math.ST" => Category::MathSt,
            "astro-ph.CO" => Category::AstroPhCo,
            "astro-ph.EP" => Category::AstroPhEp,
            "astro-ph.GA" => Category::AstroPhGa,
            "astro-ph.HE" => Category::AstroPhHe,
            "astro-ph.IM" => Category::AstroPhIm,
            "astro-ph.SR" => Category::AstroPhSr,
            "cond-mat.dis-nn" => Category::CondMatDisNn,
            "cond-mat.mes-hall" => Category::CondMatMesHall,
            "cond-mat.mtrl-sci" => Category::CondMatMtrlSci,
            "cond-mat.other" => Category::CondMatOther,
            "cond-mat.quant-gas" => Category::CondMatQuantGas,
            "cond-mat.soft" => Category::CondMatSoft,
            "cond-mat.stat-mech" => Category::CondMatStatMech,
            "cond-mat.str-el" => Category::CondMatStrEl,
            "cond-mat.supr-con" => Category::CondMatSuprCon,
            "gr-qc" => Category::GrQc,
            "hep-ex" => Category::HepEx,
            "hep-lat" => Category::HepLat,
            "hep-ph" => Category::HepPh,
            "hep-th" => Category::HepTh,
            "math-ph" => Category::MathPh,
            "nucl-ex" => Category::NuclEx,
            "nucl-th" => Category::NuclTh,
            "physics.acc-ph" => Category::PhysicsAccPh,
            "physics.ao-ph" => Category::PhysicsAoPh,
            "physics.app-ph" => Category::PhysicsAppPh,
            "physics.atm-clus" => Category::PhysicsAtmClus,
            "physics.atom-ph" => Category::PhysicsAtomPh,
            "physics.bio-ph" => Category::PhysicsBioPh,
            "physics.chem-ph" => Category::PhysicsChemPh,
            "physics.class-ph" => Category::PhysicsClassPh,
            "physics.comp-ph" => Category::PhysicsCompPh,
            "physics.data-an" => Category::PhysicsDataAn,
            "physics.ed-ph" => Category::PhysicsEdPh,
            "physics.flu-dyn" => Category::PhysicsFluDyn,
            "physics.gen-ph" => Category::PhysicsGenPh,
            "physics.geo-ph" => Category::PhysicsGeoPh,
            "physics.hist-ph" => Category::PhysicsHistPh,
            "physics.ins-det" => Category::PhysicsInsDet,
            "physics.med-ph" => Category::PhysicsMedPh,
            "physics.optics" => Category::PhysicsOptics,
            "physics.plasm-ph" => Category::PhysicsPlasmPh,
            "physics.pop-ph" => Category::PhysicsPopPh,
            "physics.soc-ph" => Category::PhysicsSocPh,
            "physics.space-ph" => Category::PhysicsSpacePh,
            "quant-ph" => Category::QuantPh,
            "nlin.AO" => Category::NlinAo,
            "nlin.CD" => Category::NlinCd,
            "nlin.CG" => Category::NlinCg,
            "nlin.PS" => Category::NlinPs,
            "nlin.SI" => Category::NlinSi,
            "q-bio.BM" => Category::QBioBm,
            "q-bio.CB" => Category::QBioCb,
            "q-bio.GN" => Category::QBioGn,
            "q-bio.MN" => Category::QBioMn,
            "q-bio.NC" => Category::QBioNc,
            "q-bio.OT" => Category::QBioOt,
            "q-bio.PE" => Category::QBioPe,
            "q-bio.QM" => Category::QBioQm,
            "q-bio.SC" => Category::QBioSc,
            "q-bio.TO" => Category::QBioTo,
            "q-fin.CP" => Category::QFinCp,
            "q-fin.EC" => Category::QFinEc,
            "q-fin.GN" => Category::QFinGn,
            "q-fin.MF" => Category::QFinMf,
            "q-fin.PM" => Category::QFinPm,
            "q-fin.PR" => Category::QFinPr,
            "q-fin.RM" => Category::QFinRm,
            "q-fin.ST" => Category::QFinSt,
            "q-fin.TR" => Category::QFinTr,
            "stat.AP" => Category::StatAp,
            "stat.CO" => Category::StatCo,
            "stat.ME" => Category::StatMe,
            "stat.ML" => Category::StatMl,
            "stat.OT" => Category::StatOt,
            "stat.TH" => Category::StatTh,
            other => return Category::other(other),
        })
    }
}

/// Serialises as the arXiv code, e.g. `"cs.LG"`, rather than as a Rust enum.
impl Serialize for Category {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

/// Deserialises through [`Category::from_str`], so a malformed code is a
/// deserialisation error instead of an `Other` that no query can use.
impl<'de> Deserialize<'de> for Category {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let code = String::deserialize(deserializer)?;
        code.parse().map_err(D::Error::custom)
    }
}

impl TryFrom<&str> for Category {
    type Error = Error;

    fn try_from(s: &str) -> Result<Self> {
        s.parse()
    }
}

static ALL: [Category; 155] = [
    Category::CsAi,
    Category::CsAr,
    Category::CsCc,
    Category::CsCe,
    Category::CsCg,
    Category::CsCl,
    Category::CsCr,
    Category::CsCv,
    Category::CsCy,
    Category::CsDb,
    Category::CsDc,
    Category::CsDl,
    Category::CsDm,
    Category::CsDs,
    Category::CsEt,
    Category::CsFl,
    Category::CsGl,
    Category::CsGr,
    Category::CsGt,
    Category::CsHc,
    Category::CsIr,
    Category::CsIt,
    Category::CsLg,
    Category::CsLo,
    Category::CsMa,
    Category::CsMm,
    Category::CsMs,
    Category::CsNa,
    Category::CsNe,
    Category::CsNi,
    Category::CsOh,
    Category::CsOs,
    Category::CsPf,
    Category::CsPl,
    Category::CsRo,
    Category::CsSc,
    Category::CsSd,
    Category::CsSe,
    Category::CsSi,
    Category::CsSy,
    Category::EconEm,
    Category::EconGn,
    Category::EconTh,
    Category::EessAs,
    Category::EessIv,
    Category::EessSp,
    Category::EessSy,
    Category::MathAc,
    Category::MathAg,
    Category::MathAp,
    Category::MathAt,
    Category::MathCa,
    Category::MathCo,
    Category::MathCt,
    Category::MathCv,
    Category::MathDg,
    Category::MathDs,
    Category::MathFa,
    Category::MathGm,
    Category::MathGn,
    Category::MathGr,
    Category::MathGt,
    Category::MathHo,
    Category::MathIt,
    Category::MathKt,
    Category::MathLo,
    Category::MathMg,
    Category::MathMp,
    Category::MathNa,
    Category::MathNt,
    Category::MathOa,
    Category::MathOc,
    Category::MathPr,
    Category::MathQa,
    Category::MathRa,
    Category::MathRt,
    Category::MathSg,
    Category::MathSp,
    Category::MathSt,
    Category::AstroPhCo,
    Category::AstroPhEp,
    Category::AstroPhGa,
    Category::AstroPhHe,
    Category::AstroPhIm,
    Category::AstroPhSr,
    Category::CondMatDisNn,
    Category::CondMatMesHall,
    Category::CondMatMtrlSci,
    Category::CondMatOther,
    Category::CondMatQuantGas,
    Category::CondMatSoft,
    Category::CondMatStatMech,
    Category::CondMatStrEl,
    Category::CondMatSuprCon,
    Category::GrQc,
    Category::HepEx,
    Category::HepLat,
    Category::HepPh,
    Category::HepTh,
    Category::MathPh,
    Category::NuclEx,
    Category::NuclTh,
    Category::PhysicsAccPh,
    Category::PhysicsAoPh,
    Category::PhysicsAppPh,
    Category::PhysicsAtmClus,
    Category::PhysicsAtomPh,
    Category::PhysicsBioPh,
    Category::PhysicsChemPh,
    Category::PhysicsClassPh,
    Category::PhysicsCompPh,
    Category::PhysicsDataAn,
    Category::PhysicsEdPh,
    Category::PhysicsFluDyn,
    Category::PhysicsGenPh,
    Category::PhysicsGeoPh,
    Category::PhysicsHistPh,
    Category::PhysicsInsDet,
    Category::PhysicsMedPh,
    Category::PhysicsOptics,
    Category::PhysicsPlasmPh,
    Category::PhysicsPopPh,
    Category::PhysicsSocPh,
    Category::PhysicsSpacePh,
    Category::QuantPh,
    Category::NlinAo,
    Category::NlinCd,
    Category::NlinCg,
    Category::NlinPs,
    Category::NlinSi,
    Category::QBioBm,
    Category::QBioCb,
    Category::QBioGn,
    Category::QBioMn,
    Category::QBioNc,
    Category::QBioOt,
    Category::QBioPe,
    Category::QBioQm,
    Category::QBioSc,
    Category::QBioTo,
    Category::QFinCp,
    Category::QFinEc,
    Category::QFinGn,
    Category::QFinMf,
    Category::QFinPm,
    Category::QFinPr,
    Category::QFinRm,
    Category::QFinSt,
    Category::QFinTr,
    Category::StatAp,
    Category::StatCo,
    Category::StatMe,
    Category::StatMl,
    Category::StatOt,
    Category::StatTh,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taxonomy_is_complete() {
        assert_eq!(Category::all().len(), 155);
    }

    #[test]
    fn every_category_round_trips() {
        for category in Category::all() {
            let parsed: Category = category.as_str().parse().unwrap();
            assert_eq!(&parsed, category, "{} did not round-trip", category);
            assert!(
                !matches!(parsed, Category::Other(_)),
                "{} fell through to Category::Other",
                category
            );
        }
    }

    #[test]
    fn codes_are_unique() {
        let mut codes: Vec<&str> = Category::all().iter().map(Category::as_str).collect();
        let total = codes.len();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), total, "duplicate category codes");
    }

    #[test]
    fn unknown_but_well_formed_codes_survive_as_other() {
        let c: Category = "cs.NOTAREALCATEGORY".parse().unwrap();
        assert_eq!(c, Category::other("cs.NOTAREALCATEGORY").unwrap());
        assert_eq!(c.as_str(), "cs.NOTAREALCATEGORY");
        assert_eq!(c.to_string(), "cs.NOTAREALCATEGORY");
    }

    #[test]
    fn malformed_codes_are_rejected_rather_than_becoming_other() {
        for code in [
            "",
            " ",
            "cs.",
            ".AI",
            "CS.AI",
            "cs.A.I",
            r#"cs.AI" OR cat:"cs.LG"#,
            "cs AI",
            "1cs.AI",
            &"x".repeat(MAX_CODE_LEN + 1),
        ] {
            let parsed = code.parse::<Category>();
            assert!(parsed.is_err(), "{code:?} should not parse");
            assert!(Category::other(code).is_err(), "{code:?} should not parse");
            assert!(
                CategoryCode::new(code).is_err(),
                "{code:?} should not parse"
            );
        }
    }

    #[test]
    fn every_real_code_shape_is_accepted() {
        for category in Category::all() {
            assert!(
                is_well_formed_code(category.as_str()),
                "{} was rejected by the grammar check",
                category
            );
        }
    }

    #[test]
    fn serde_uses_the_arxiv_code_and_validates_on_the_way_in() {
        let json = serde_json::to_string(&Category::CsLg).unwrap();
        assert_eq!(json, r#""cs.LG""#);
        assert_eq!(
            serde_json::from_str::<Category>(&json).unwrap(),
            Category::CsLg
        );

        // Unknown but well-formed survives the round trip.
        let other = Category::other("cs.FUTURE").unwrap();
        let json = serde_json::to_string(&other).unwrap();
        assert_eq!(json, r#""cs.FUTURE""#);
        assert_eq!(serde_json::from_str::<Category>(&json).unwrap(), other);

        // Malformed no longer sneaks in through Deserialize.
        assert!(serde_json::from_str::<Category>(r#""not a category""#).is_err());
        assert!(serde_json::from_str::<Category>(r#""cs.AI\" OR cat:\"cs.LG""#).is_err());
    }

    #[test]
    fn every_category_that_can_be_built_round_trips_through_serde() {
        // `Other` used to hold a bare String, so a value could be built —
        // and serialised — that Deserialize then rejected.
        let mut values: Vec<Category> = Category::all().to_vec();
        values.push(Category::other("cs.FUTURE").unwrap());
        values.push(Category::other("brand-new").unwrap());

        for category in values {
            let json = serde_json::to_string(&category).unwrap();
            let back: Category = serde_json::from_str(&json).unwrap();
            assert_eq!(back, category, "{category} did not round-trip");
        }
    }

    #[test]
    fn categories_missing_before_2_0_are_present() {
        // Regression: the 1.x enum stopped at cs.IR, silently making most of
        // arXiv unqueryable.
        for code in [
            "cs.IT",
            "cs.LO",
            "cs.MA",
            "cs.NE",
            "cs.NI",
            "cs.RO",
            "cs.SE",
            "cs.SI",
            "cs.SY",
            "stat.ML",
            "eess.SP",
            "math-ph",
            "q-bio.NC",
            "econ.EM",
            "astro-ph.CO",
        ] {
            let c: Category = code.parse().unwrap();
            assert!(!matches!(c, Category::Other(_)), "{code} is still missing");
        }
    }
}
