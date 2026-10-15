//! Which subrecords are lstrings, per record type, measured on the corpus: since M3.E4 a
//! cross-check of the schema and a fallback where the schema has no member (Fable review
//! round 2, finding 2).
//!
//! `set`, `build` and `validate` take lstrings from the schema first (`edit::fields`
//! layouts; every lstring member wherever it sits in a payload, through
//! `kitkraft_doc::typed::lstring_spans`). Lane B typed all 105 lstring signatures of this
//! table (M3.B8): 100 are whole-subrecord schema members, and [`compare`] checks that
//! the two agree. The other 5 (CHAL `FULL`/`DESC`, GBFM `HULL`, PNDT and STDT `FULL`) sit
//! inside the base form component blocks the schema keeps raw, so this table is the
//! fallback there and every use names it in a limitation ([`table_note`]). A localized
//! plugin stores every lstring as a 4-byte string ID, so an lstring nothing types would be
//! copied into a non-localized patch as four bytes the game reads as text.
//!
//! **Source** (`corpus:` the 13 official ESMs with English string tables, confidence
//! `corpus`, ADR 0004 "Owner approval" clause 3): the pinned xEdit file is not available
//! locally (`plan/STATUS.md`, Blockers), so the table is built from the corpus by
//! `tests/lstring_table.rs` (`lstring_table_matches_the_corpus`, which also checks this
//! table against a fresh scan). Per record type and subrecord signature, over the distinct
//! non-zero values in each file:
//!
//! - **lstring**: exactly 4 bytes in every occurrence; at least [`MIN_SAMPLES`] distinct
//!   values; more than 95% of them are IDs in the plugin's own English
//!   `.STRINGS`/`.DLSTRINGS`/`.ILSTRINGS`; more than 98% of those in one kind of table (the
//!   game reads a field from one kind); and at most 1% are also the ID of another lstring
//!   of the same plugin (anchors: `FULL`/`DESC` in a first round, then every lstring the
//!   first round found).
//! - **not an lstring**: some occurrence is not 4 bytes (a localized file stores every
//!   lstring as a 4-byte ID); or at most 50% resolve; or the resolving ones spread over the
//!   kinds (at most 90% in one); or more than 10% are another lstring's ID. String IDs and
//!   FormIDs share dense ranges in `Starfield.esm`, so a FormID or small-integer field
//!   resolves by coincidence (up to 97% for a field with one repeated value); the
//!   one-kind and anchor tests tell those apart.
//! - **unclassified**: the rest (too few values, only zeros, between the thresholds), and
//!   every pair the corpus does not have.
//!
//! The method cannot see an lstring inside a larger struct (such a subrecord is never 4
//! bytes); the schema's typed lstrings cover those wherever they sit. `docs/validate.md`
//! ("lstring signature table") records the method and its limits.

/// Fewest distinct non-zero values that make a 4-byte signature an lstring.
pub const MIN_SAMPLES: u64 = 5;

/// The lstrings every record type shares (`FULL`, `DESC`: `uesp-skyrim` fact in
/// `plan/execution-plan.md`, typed by lane B with corpus evidence). Their string IDs are
/// the anchor set: a value of another signature that is also a `FULL`/`DESC` ID of another
/// record is a coincidence, not that signature's own string.
pub fn is_anchor(sub: [u8; 4]) -> bool {
    matches!(&sub, b"FULL" | b"DESC")
}

/// What the corpus says about one (record type, subrecord) pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// A string ID in a localized plugin.
    LString,
    /// Not an lstring.
    NotLString,
    /// The corpus cannot tell (or never saw it).
    Unclassified,
}

/// Corpus counts for one (record type, subrecord) pair.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    /// Occurrences.
    pub occurrences: u64,
    /// Occurrences exactly 4 bytes long.
    pub four_bytes: u64,
    /// 4-byte occurrences whose value is not 0.
    pub nonzero: u64,
    /// Distinct non-zero values (summed per file).
    pub distinct: u64,
    /// Distinct values that are IDs in the plugin's string tables.
    pub resolved: u64,
    /// Resolved values by kind of table: `.STRINGS`, `.DLSTRINGS`, `.ILSTRINGS`.
    pub kinds: [u64; 3],
    /// Distinct values that are also FormIDs stored in the same plugin (diagnostic).
    pub form_ids: u64,
    /// Distinct values that are also the string ID of a `FULL`/`DESC` of another record
    /// type in the same plugin ([`is_anchor`]).
    pub anchored: u64,
}

/// Whether `part` is more than `percent`% of `whole`.
fn over(part: u64, whole: u64, percent: u64) -> bool {
    u128::from(part).saturating_mul(100) > u128::from(whole).saturating_mul(u128::from(percent))
}

/// The class the thresholds in the module docs give the counts `c` of signature `sub`.
pub fn classify(sub: [u8; 4], c: &Counts) -> Class {
    if c.four_bytes < c.occurrences {
        return Class::NotLString;
    }
    if c.distinct == 0 {
        return Class::Unclassified;
    }
    let one_kind = c.kinds.iter().copied().max().unwrap_or(0);
    let lstring = c.distinct >= MIN_SAMPLES
        && over(c.resolved, c.distinct, 95)
        && over(one_kind, c.resolved, 98)
        && (is_anchor(sub) || !over(c.anchored, c.distinct, 1));
    if lstring {
        Class::LString
    } else if !over(c.resolved, c.distinct, 50)
        || !over(one_kind, c.resolved, 90)
        || (!is_anchor(sub) && over(c.anchored, c.distinct, 10))
    {
        Class::NotLString
    } else {
        Class::Unclassified
    }
}

/// `(record type, lstring signatures, other signatures, unclassified signatures)`, each list
/// space-separated, generated by `tests/lstring_table.rs` from the corpus (module docs).
pub const TABLE: &[(&str, &str, &str, &str)] = &[
    ("AACT", "", "CNAM DNAM EDID", "FNAM TNAM"),
    ("AAMD", "", "EDID SNAM", ""),
    ("AAPD", "", "AAAP AAPS ANAM BNAM EDID RADR WTMX", ""),
    ("ACHR", "", "BFCB BFCE DATA EDID KSIZ KWDA NAME OPDS VMAD XALG XEED XESP XEZN XGDS XIS2 XLCM XLCN XLKR XLKT XLRT XLYR XOWN XPCS XRFG XRGB XRGD XSCL", "XHLT XRNK"),
    ("ACTI", "ATTX FULL", "ACSH ALSH ANAM BFCB BFCE BNAM CITC CNAM CTDA DEST DMDL DSDL DSTA DSTD DSTF EDID ENAM FLCS FLLD FNAM FTYP INAM JNAM KSIZ KWDA MODL MOLM NTRM NVNM OBND ODTY OPDS PNAM PRPS PTCL PTT2 REFL SNBH SNTP STCP VMAD WMAT WTFM XALG XFLG", "DMDS"),
    ("ADDN", "", "BFCB BFCE DATA DNAM EDID FLLD LNAM MODL OBND PSDF REFL", "ODTY"),
    ("AFFE", "", "ANAM BNAM CNAM DNAM EDID ENAM FNAM NLDT NNAM PNAM RNAM", ""),
    ("ALCH", "DESC DNAM FULL", "BFCB BFCE CDIX CTDA CUSH CVPA DATA EDID EFID EFIF EFIT ENIT FLLD KSIZ KWDA MNAM MODL MOLM MUID OBND OPDS PDSH PTT2 PUSH REFL XALG XFLG ZNAM", "ODTY"),
    ("AMBS", "", "ASAE ASAS ASMB EDID WED0", ""),
    ("AMDL", "", "ANAM EDID", ""),
    ("AMMO", "FULL ONAM", "DATA DNAM EDID FLLD KSIZ KWDA MODL NAM1 OBND PDSH PTT2 PUSH", "DESC ODTY"),
    ("ANIO", "", "ANAM BFCB BFCE BNAM EDID FLLD MODL REFL XALG", ""),
    ("AOPF", "", "EDID OBND OBSV OCCV", "ODTY"),
    ("AOPS", "", "ANAM EDID", ""),
    ("AORU", "", "AOR2 EDID", ""),
    ("ARMA", "", "ANAM BFCB BFCE BNAM BO64 BSMP DNAM EDID FLLD KSIZ KWDA MLM1 MLM2 MLM3 MLM4 MNAM MO2F MO3F MO4F MO5F MOD2 MOD3 MOD4 MOD5 MOD6 MODL NAM4 NAM5 NAM6 NAM7 ONAM PNAM REFL RNAM SNAM SNDD TNAM VNAM XFLG", ""),
    ("ARMO", "", "AFSG ANAM APPR AVSG BAMT BFCB BFCE BNAM BO64 DAMA DATA EDID EITM FLLD FNAM INDX INRD KSIZ KWDA MLM2 MO2F MO4F MOD2 MOD4 MODL OBND OBTE OBTF OBTS PDSH PTT2 PUSH QUPA REFL RNAM STOP VMAD XFLG", "DESC FULL ODTY"),
    ("ARTO", "", "ANAM BFCB BFCE BNAM DNAM EDID ENAM FLLD KSIZ KWDA MNAM MODL MOLM OBND OPDS PTCL PTT2 REFL XALG XFLG", "ODTY"),
    ("ASPC", "", "AAMB AEAR AMUS ASDF ASLS BNAM BOLV DEVT EDID FLTV OBND OPDS WED0 WED1 XTRI", "ODTY"),
    ("ATMO", "", "EDID RDIF REFL RFDP", ""),
    ("AVIF", "ANAM FULL", "AVFL EDID NAM0 NAM1 NAM2 NAM3 NLDT", ""),
    ("AVMD", "", "AVMP BFCB BFCE EDID FLLD GNAM ITMC LNAM MNAM MODL MODT NNAM TNAM VNAM YNAM", ""),
    ("BIOM", "FULL", "ANAM BDFS BIAS BIMT BMC1 BMC2 BMC3 BNAM BTPS CNAM DNAM EDID EFSD ENAM FNAM GNAM INAM JNAM KNAM KSIZ KWDA LNAM MNAM NAM0 NAM1 NAM2 NAM3 NAM4 NNAM ONAM PNAM RNAM SNAM TNAM TX00 TX16 UNAM UX00 VX00 WNAM WX00 XX00 YNAM YX00", ""),
    ("BMMO", "", "BFCB BFCE CTDA EDID FLLD KNAM KSIZ KWDA LNA2 LNAM MODL OBND OPDS REFL", "ODTY"),
    ("BMOD", "", "BFCB BFCE DATA EDID FLLD FLTR INTV MODL", ""),
    ("BNDS", "", "DNAM EDID MNAM OBND", "ODTY"),
    ("BOOK", "DESC ENAM FNAM FULL", "BFCB BFCE DATA DNAM EDID FLLD GNAM INAM KSIZ KWDA MODL MOLM OBND PDSH PTT2 PUSH REFL VMAD XALG XFLG", "CNAM ODTY"),
    ("BPTD", "BPTN", "BPD2 BPNN BPNT DNAM EDID ENAM FLLD FNAM INAM MODL NAM4", ""),
    ("CAMS", "", "CTDA DATA EDID FLLD GNAM KSIZ KWDA MNAM NLDT", ""),
    ("CELL", "FULL", "BFCB BFCE DATA EDID LTMP MHDT MOD2 REFL TODD XBPS XCAS XCCM XCGD XCIB XCIM XCLA XCLC XCLD XCLL XCLW XCMO XCWM XCWT XEMP XILS XLCN XLKR XLKT XOWN XTV2 XWCN XWCU XWEM", ""),
    ("CHAL", "DESC FULL", "ANAM ATAF ATAN ATAV ATCP BFCB BFCE CITC CTDA DNAM EDID", ""),
    ("CLAS", "FULL", "DATA EDID PRPS", "DESC"),
    ("CLDF", "", "EDID REFL", ""),
    ("CLFM", "", "CNAM EDID", "FNAM"),
    ("CLMT", "", "EDID TNAM WLST WSLT", ""),
    ("CNDF", "", "CIS1 CIS2 CTDA EDID PNAM QNAM", ""),
    ("COBJ", "DESC", "ANAM BFCB BFCE BNAM CNAM CTDA CUSH DATA EDID FLTR FNAM FVPA INTV JNAM LRNM NNAM PDSH PUSH RECF RQPK SNAM TNAM", ""),
    ("COLL", "", "BNAM CNAM EDID FNAM GNAM MNAM NLDT", ""),
    ("CONT", "FULL", "ANAM APPR BFCB BFCE BNAM CNAM CNTO COCT DATA DCED DCSD DEST DSTD DSTF EDID FLLD FTYP KSIZ KWDA MODL MOLM NTRM OBND ODTY ONAM OPDS PRPS PTCL PTT2 REFL SNBH SNTP STOP VMAD WED0 WED1 XALG XFLG", ""),
    ("CPTH", "", "ANAM CTDA DATA EDID SNAM", ""),
    ("CSTY", "", "CSCR CSCV CSFL CSGD CSLR CSME CSRA CSSA CSSD CSSG CSSM CSSR CSTN DATA EDID FNAM TNAM UNAM", ""),
    ("CUR3", "", "EDID REFL", ""),
    ("CURV", "", "EDID REFL", ""),
    ("DEBR", "", "DATA EDID MODT", ""),
    ("DFOB", "", "DATA EDID XALG", ""),
    ("DIAL", "FULL", "ANAM BFCB BFCE BNAM DATA EDID FLCS KNAM OPDS PNAM QNAM SNAM TIFC TIFL", ""),
    ("DLBR", "", "DNAM EDID QNAM SNAM", "TNAM"),
    ("DMGT", "FULL", "DNAM EDID", ""),
    ("DOBJ", "", "DNAM EDID", ""),
    ("DOOR", "FULL", "ANAM BFCB BFCE BNAM CNAM DCSH DEVT DLSH DOSH EDID ENAM FLLD FLTR FNAM FTYP INTV KSIZ KWDA MODL MOLM NTRM NVNM OBND ODTY PRPS REFL STCP VMAD XALG XFLG", "ONAM"),
    ("EFSH", "", "ANAM BFCB BFCE BNAM DATA DNAM EDID ENAM FLLD MODL PTCL", ""),
    ("EFSQ", "", "EDID REFL", ""),
    ("ENCH", "FULL", "BFCB BFCE CTDA EDID EFID EFIF EFIT ENIT MNAM MUID OBND PTCL", "ODTY"),
    ("EQUP", "", "ANAM DATA EDID PNAM", ""),
    ("EXPL", "FULL", "ANAM BFCB BFCE BNAM DAMA DEFL EDID EITM ENAM FLLD MNAM MODL OBND OPDS PTCL REFL XALG", "ODTY"),
    ("FACT", "FULL", "BFCB BFCE CRGP CRGR CRHR CRVA DATA DNAM EDID FLCS GRPH HERD PLVD PRIS VENC VEND VENV VTCK XNAM", ""),
    ("FFKW", "", "EDID REFL", ""),
    ("FLOR", "ATTX FULL", "ANAM APPR BFCB BFCE BNAM DEST DSTD DSTF EDID FHLS FLLD FLXP FMAG FMAH FMIG FMIH FNAM JNAM KSIZ KWDA MODL MOLM OBND OBTE OBTF OBTS ODTY OPDS PFHS PFIG PFPC PNAM PRPS PTCL STOP VMAD XFLG", ""),
    ("FLST", "FULL", "ANAM BFCB BFCE CITC CTDA EDID FLTR INAM INTV LNAM", ""),
    ("FOGV", "", "EDID REFL", ""),
    ("FORC", "", "EDID REFL", ""),
    ("FSTP", "", "ANAM DATA EDID", ""),
    ("FSTS", "", "DATA EDID XCNT", ""),
    ("FURN", "ATTX FULL", "ALSH ANAM BFCB BFCE BNAM CNAM DEST DMDL DSTD DSTF EDID FLLD FNAM FNPR FTMP FTYP GNAM INAM JNAM KSIZ KWDA MNAM MODL MOLM NNAM OBND OPDS PNAM PRPS PTCL PTT2 REFL SNAM SNBH SNTP STOP VCCD VCMT VCSB VCTT VMAD VMRT VWWD WBDT XALG XFLG XMRK", "ODTY"),
    ("FXPD", "FULL", "EDID MNAM MWGT", ""),
    ("GBFM", "FULL HULL", "ANAM APPR BFCB BFCE BLUF BNAM BODC BODM BODS BODV BOID BUO4 CDND CDNS CITC CNAM CNTO COCT CTDA DEST DMDL DNAM DSTD DSTF EDID ESSF EXAC EXAS EXBS EXCI EXCN EXDC EXDZ FCTF FLFM FLKW FLLD FLTR FLTV FTYP INAM INTV ITMC KSIZ KWDA MODL NVNM OBND OBTE OBTF OBTS ODTY OPDS PRPS SAIA SHWB STCP STOP STRD STRV VMAD", "SODA"),
    ("GBFT", "", "EDID STRV", ""),
    ("GCVR", "", "BFCB BFCE DNAM EDID GNAM KSIZ KWDA LNAM YNAM", ""),
    ("GLOB", "", "BFCB BFCE EDID FLCS FLTV", ""),
    ("GMST", "", "EDID XALG", "DATA"),
    ("GPOF", "DNAM NNAM RESN VOVS", "EDID GPOD TNAM VNAM VORN VORV WNAM", ""),
    ("GPOG", "", "BNAM EDID GOGL", "NNAM"),
    ("GRAS", "", "BFCB BFCE DNAM EDID FLLD KSIZ KWDA MODL OBND", "ODTY"),
    ("GWED", "", "EDID REFL", ""),
    ("HAZD", "FULL", "ANAM BFCB BFCE BNAM CTDA DNAM EDID FLLD MNAM MODL OBND PTCL REFL XALG", "ODTY"),
    ("HDPT", "FULL", "BFCB BFCE DATA EDID FLLD HNAM MNAM MODL NAM2 NAM3 PNAM RNAM TNAM", ""),
    ("IDLE", "FULL", "ANAM CIS1 CTDA DNAM EDID ENAM FNAM GNAM", ""),
    ("IDLM", "", "EDID FLLD IDLA IDLC IDLF IDLT KSIZ KWDA MODL OBND PNAM QNAM", "ODTY"),
    ("IMAD", "", "EDID REFL", ""),
    ("IMGS", "", "EDID RDIF REFL RFDP", ""),
    ("INFO", "NAM1 RNAM", "ANAM BFCB BFCE BNAM CIS2 COCT CTDA DNAM DOFT DPLT EDID ENAM FLCS FNAM GNAM HNAM HTID INAM INTV LVCR MODQ NAM0 NAM2 NAM3 NAM4 NAM8 NAM9 OCOR PERK PLRL PNAM QUAL RVSH SCSP STRV TIQS TRDA TROT TSCE VCLR VMAD WED0 XNAM", "ATAC FLAV FLMV"),
    ("INNR", "WNAM", "EDID KSIZ KWDA UNAM VNAM XNAM YNAM", ""),
    ("IPCT", "", "BFCB BFCE DATA DODT EDID FLLD FNAM GNAM HNAM IDP1 IDP3 IDSH MODL PTCL REFL", ""),
    ("IPDS", "", "EDID ENAM PNAM", ""),
    ("IRES", "FULL NNAM", "CNAM CUSH EDID FNAM GNAM KSIZ KWDA NAM1 NAM2 NAM3 SNAM TINC", ""),
    ("KEYM", "FULL", "DATA EDID FLAG FLLD KSIZ KWDA MODL MOLM OBND PTT2 VMAD XALG XFLG", "ODTY"),
    ("KSSM", "", "EDID KNAM RSMC RSMH WED0", ""),
    ("KYWD", "FULL", "BFCB BFCE CNAM DATA DNAM EDID ENAM FNAM TNAM WAIM WFIR", ""),
    ("LAYR", "", "BFCB BFCE EDID FLCS LODB PNAM XCLP", ""),
    ("LCRT", "", "BFCB BFCE CNAM EDID FLTR INTV", "FNAM TNAM"),
    ("LCTN", "FULL", "ACEP ACID ACPR ACSR ACUN ACUR ANAM CNAM DATA EDID FNAM KSIZ KWDA LCEC LCEP LCID LCPR LCSR LCUN LCUR MNAM NAM1 PNAM PRPS TNAM XNAM YNAM", "RNAM"),
    ("LENS", "", "CNAM DNAM EDID ENAM FNAM LFSD LFSP MNAM XNAM", ""),
    ("LGDI", "", "ANAM BNAM CITC CNAM CTDA DATA DNAM EDID ENAM FLLD FNAM GNAM HNAM JNAM KNAM LNAM MNAM OBND", "FULL ODTY"),
    ("LGTM", "", "DALC DATA EDID", ""),
    ("LIGH", "", "ANAM BFCB BFCE BNAM DAT2 EDID FLAD FLBD FLCS FLGD FLLD FLRD FLTR FVLD INTV KSIZ KWDA LLLD LNAM MODL NAM0 OBND REFL", "ODTY"),
    ("LMSW", "", "BFCB BFCE EDID FLCS FLTR INTV KSIZ KWDA REFL", ""),
    ("LSCR", "DESC", "CTDA EDID ICON", ""),
    ("LTEX", "", "BNAM EDID HNAM MNAM QNAM", ""),
    ("LVLB", "", "CTDA EDID FLLD LLCT LVLD LVLF LVLM LVLO MODL OBND VMAD", "ODTY"),
    ("LVLI", "ONAM", "BFCB BFCE CTDA EDID FLLD FTYP LIMC LLCT LLKC LVLD LVLF LVLG LVLL LVLM LVLO MODL OBND OPDS VMAD XALG", "ODTY"),
    ("LVLN", "", "BFCB BFCE CTDA EDID FLLD LLCT LVLF LVLM LVLO MODL OBND VMAD", "LVLD ODTY ONAM"),
    ("LVLP", "", "BFCB BFCE CTDA EDID FLLD FTYP LLCT LVLD LVLF LVLG LVLM LVLO MODL NAM4 NAM8 OBND OPDS VNAM YNAM", "CITC NAM1 NAM2 NAM3 NAM5 NAM6 NAM7 NAM9 ODTY UNAM WNAM XNAM ZNAM"),
    ("LVSC", "", "EDID LLCT LVLF LVLM LVLO", "LVLD"),
    ("MAAM", "", "EDID SNAM", ""),
    ("MATT", "", "ANAM BNAM CNAM EDID FLLD FNAM HNAM MNAM PNAM", ""),
    ("MESG", "DESC FULL ITXT NNAM", "BNAM CTDA DNAM DODT EDID IBIN QNAM TNAM", "INAM"),
    ("MGEF", "DNAM FULL", "BFCB BFCE CTDA DATA EDID ESSH KSIZ KWDA REFL VMAD", ""),
    ("MISC", "FULL NNAM", "ANAM BFCB BFCE BNAM CNAM CUSH CVPA DAMC DATA DEST DMDL DSTD DSTF EDID FIMD FLAG FLLD KSIZ KWDA MODL MOLM OBND OPDS PDSH PTT2 PUSH REFL VMAD XALG XFLG", "ODTY"),
    ("MOVT", "", "BOLV EDID KNAM LNAM MNAM SPED", "INTV"),
    ("MRPH", "", "EDID MOBC TCMP TMPP", ""),
    ("MSTT", "FULL", "ANAM BFCB BFCE BNAM CNAM DAMC DATA DEFL DEST DMDL DSDL DSTA DSTD DSTF EDID FLCS FLLD INAM KSIZ KWDA MODC MODL MOLM MSLS MSMO OBND ODTY OPDS PRPS PTCL PTT2 REFL SNBH SNTP SODA VMAD XALG XFLG", "DMDS"),
    ("MTPT", "", "EDID REFL", ""),
    ("MUSC", "", "EDID FNAM PNAM TNAM UNAM VNAM WNAM", ""),
    ("MUST", "", "CITC CNAM CTDA DNAM EDID FLTV FNAM MSTF MTSH SNAM", ""),
    ("NAVI", "", "NVER NVMI NVPP", ""),
    ("NAVM", "", "BFCB BFCE MNAM NVNM REFL VMAD", ""),
    ("NOCM", "", "DATA EDID INDX INTV NAM1", ""),
    ("NPC_", "ATTX FULL LNAM SHRT", "ACBS AIDT APPR ATKR BCOL BFCB BFCE BMPN BMPV BNAM CNAM CNTO COCT CRIF CS3E CS3F CS3H CS3S CSCR DATA DNAM DOFT DPLT ECOL ECOR EDCT EDID FHCL FLEE FLTR FMRG FMRI FMRS FMSI FTYP HCOL HEFA INAM INTV JCOL KSIZ KWDA LTPC LTPT MNAM MRSV MWGT NAM4 NAM5 NAM6 NNAM NTRM OBND OBTE OBTF OBTS ONA2 OPDS PKID PNAM PRKR PRKZ PRON PRPS PTT2 QNAM QSTA RDSA RNAM SNAM SNTP SOFT SPLO SPOR STCP STON STOP TETC TNAM TPLT TPTA VMAD VNAM VTCK WNAM XALG ZNAM", "ODTY"),
    ("OMOD", "DESC FULL", "BFCB BFCE DATA EDID FLLD FNAM KSIZ KWDA LNAM MNAM MODL NAM1 XFLG", ""),
    ("OSWP", "", "EDID", ""),
    ("OTFT", "", "EDID INAM", ""),
    ("OVIS", "", "DATA EDID INDX", ""),
    ("PACK", "", "ANAM BNAM CIS1 CIS2 CITC CNAM CTDA EDID FLAV FNAM IDLA IDLC IDLF IDLT INAM NNAM PDTO PFO2 PKC2 PKCU PKDT PLDT PNAM POBA POCA POEA PRCB PSDT PTDA QNAM STSC UNAM VMAD XNAM", ""),
    ("PCBN", "", "BFCB BFCE CITC CTDA EDID KWDA NAM1 NAM2 NAM3 NAM4 NAM5 NAM8 NAM9 PRTN PRVN VNAM WNAM XNAM YNAM", "NAM6 NAM7 UNAM ZNAM"),
    ("PCCN", "", "BFCB BFCE CITC CTDA EDID IOVR KWDA NAM1 NAM2 NAM3 NAM4 NAM8 NAM9 PCCC PRTN PRVN VNAM XNAM YNAM", "NAM5 NAM6 NAM7 UNAM WNAM ZNAM"),
    ("PCMT", "", "CITC CTDA EDID NAM2 NAM5", "NAM1"),
    ("PDCL", "", "BFCB BFCE DATA DODT EDID KSIZ KWDA OBND OPDS PTT2 REFL SNBH", "ODTY"),
    ("PERK", "DESC EPF2 FULL", "ANAM ATAF ATAN ATAV ATCP CIS1 CITC CTDA DATA DNAM EDID EPF3 EPFB EPFD EPFT GNAM PRKC PRKE PRKF PRRF PRRK PRUC RNAM TNAM UNAM VMAD", ""),
    ("PERS", "", "DAT2 DATA", ""),
    ("PGRE", "", "DATA NAME VMAD XESP XLRT XLYR XOWN XRFG XRGD XSCL", ""),
    ("PHZD", "", "DATA EDID NAME VMAD XESP XLKR XLKT XLYR XPCS XRFG XSCL", ""),
    ("PKIN", "FULL", "ANAM BFCB BFCE BNAM CITC CNAM CTDA EDID FLTR FNAM FTYP KSIZ KWDA MOLM NAM1 NAM2 NAM4 NAM6 NAM7 NAM8 NTRM OBND ODTY OPDS PRPS PTT2 REFL SNBH SNTP VMAD VNAM XALG YNAM", "NAM3 NAM5 NAM9 UNAM WNAM XNAM ZNAM"),
    ("PMFT", "FULL", "EDID FNAM HNAM INAM", ""),
    ("PNDT", "FULL", "ANAM BDED BDST BFCB BFCE CNAM DENS DNAM EDID ENAM EOVR FLLD FNAM GNAM HNAM INAM KNAM KSIZ KWDA MCQP MODL NNAM PCCC PHLA PPBD RSCS TEMP XLMS XMPM XMSP", ""),
    ("PROJ", "FULL", "ANAM BFCB BFCE BNAM DATA DEST DSTD DSTF EDID FLLD MODL NAM1 NAM3 OBND OPDS PRAS PRCS PROD PTCL REFL VNAM XALG", "ODTY"),
    ("PSDC", "", "EDID REFL", ""),
    ("PTST", "", "EDID", ""),
    ("QUST", "CNAM FULL NNAM QMDP QMDS QMDT QMSU", "ALAM ALCA ALCC ALCL ALCM ALCO ALCS ALDI ALDN ALEA ALED ALEQ ALFA ALFC ALFD ALFE ALFG ALFI ALFL ALFR ALFV ALID ALKF ALLA ALLR ALLS ALMI ALNA ALNR ALNT ALPC ALPN ALPS ALRT ALSP ALST ALSY ALTM ALUA ALUB ANAM BFCB BFCE CIS1 CIS2 CNTO COCT CTDA DNAM ECOR EDID ENAM FLLD FLTR FNAM FTYP GNAM GWOR INDX KNAM KSIZ KWDA LNAM NAM1 NAM2 NAM3 NEXT OCOR QDUP QMTY QOBJ QRCR QRRD QRXP QSDT QSRC QSRD QSTA QTGL QTLM QTYP SCCM SCFC SPOR VMAD VTCK", ""),
    ("RACE", "DESC FDSL FMRN FULL SNAM", "ANAM ATKD ATKE ATKS ATKT ATKW BFCB BFCE BNAM BO64 BSTT DAT2 DNAM EDID ENAM FCTP FDSI FLLD FLMV FMRI FMRS FMRU FMSR FNAM FSTT GNAM HEAD HSTT INDX KSIZ KWDA MNAM MPGM MPGN MSSA MSSI MSSS MTNM MTYP NAM0 NAM1 NAM3 NAM4 NAM5 NAM6 NAME NNAM NTOP PNAM PRPS PTOP QNAM QSTI RBPC RNAM RPRF RPRM SADD SAKD SAPT SGNM SPED SPLO SRAC SRAF STCP STKD SWMV UNAM UNWP VNAM VTCK WED0 WED1 WKMV WNAM ZNAM", ""),
    ("REFR", "FULL UNAM", "BFCB BFCE BOLV DATA EDID FLCS FLTR FNAM GNAM HNAM INAM INTV JNAM KSIZ KWDA LNAM MNAM NAM4 NAM8 NAME ONAM OPDS PDTO PNAM PTCL REFL SNAM TNAM TODD VISI VLMS VMAD VNAM XACT XALD XALG XATP XATR XBPO XBSD XCDD XCNT XCOL XCZC XCZR XDTF XDTS XEED XEMI XESP XEZN XFLG XGDS XGOM XHTW XIS2 XLBD XLCD XLCM XLCN XLFD XLGD XLIB XLIG XLKR XLKT XLLD XLMS XLOC XLRD XLRT XLSM XLVD XLYR XMRK XNDP XNSE XOWN XPCK XPCS XPDD XPDO XPLK XPPA XPPS XPRD XPRM XRDS XRFG XRGD XSAD XSCL XSL1 XTEL XTNM XTV2 XVL2 XVOI XWCN XWCU XWPK YNAM", "BNAM CITC NAM1 NAM2 NAM3 NAM5 NAM6 NAM7 NAM9 WNAM XCZA XHLT XNAM XRNK ZNAM"),
    ("REGN", "", "EDID RCLR RDAT RDWT RPLD RPLI", ""),
    ("REVB", "", "ANAM EDID RABG", ""),
    ("RFGP", "", "BFCB BFCE FLCS KSIZ KWDA LNAM MNAM PNAM RNAM VMAD", ""),
    ("RSGD", "", "DNAM EDID RNAM", ""),
    ("RSPJ", "DESC FULL", "BNAM CNAM EDID FVPA KNAM NNAM RNAM RQPK SNAM TNAM", ""),
    ("SCCO", "", "EDID QNAM SNAM VNAM XNAM", ""),
    ("SCEN", "", "ACBS AIDT ALID ALLA ANAM ATTR ATXT BIPL BNAM BOLV BTXT CIS1 CIS2 CITC CNAM CTDA DATA DEVT DMAX DMIN DNAM DOFT DTGT EDID ENAM ESCE ESCS FLAV FLMV FNAM HNAM HTID INAM INTT JNAM LCEP LNAM LVCR LVLO MPCD NAM0 NEXT NNAM NVCI OCOR PAPI PAPN PASP PLRL PNAM PNST PPST QUAL REPL SCPI SCPP SCQS SCSP SHRT SNAM SNOT SOFT SPDF SPEX SPKW SPKY SPMA SPMV SPOR SPPK SPPQ SPRK SPRP SPRW SSPN STRV TNAM UNAM VCLR VENC VMAD VNAM VNML VTXT WED0 WED1 WNAM XNAM", "ATAC"),
    ("SCOL", "", "BFCB BFCE DATA EDID FLLD FLTR MODL OBND ONAM SNTP", "ODTY"),
    ("SDLT", "", "ACTV DAMA EDID ITMC", ""),
    ("SECH", "", "ANAM BNAM CNAM DNAM ECHD ECHE ECHO ECSH ECTE EDID ENAM NNAM OBND XALG", "ODTY"),
    ("SFBK", "", "ANAM BFCB BFCE DAT2 DATA DNAM EDID ENAM FNAM GNAM HNAM INAM JNAM KNAM NAM0 NAM1 NAM2 NAM3 NAM4 NAM5 WHGT", ""),
    ("SFPC", "", "BNAM CNAM DNAM EDID ENAM", ""),
    ("SFPT", "", "BFCB BFCE CNAM DAT2 DATA DNAM EDID FNAM GNAM", ""),
    ("SFTR", "", "BFCB BFCE CNAM DAT2 DNAM EDID FNAM GNAM NAM1", ""),
    ("SMBN", "", "CITC CTDA DNAM EDID PNAM SNAM XNAM", ""),
    ("SMEN", "", "EDID ENAM PNAM SNAM", "CITC DNAM XNAM"),
    ("SMQN", "", "CITC CTDA DNAM EDID HNAM MNAM NNAM PNAM QNAM RNAM SNAM XNAM", ""),
    ("SOUN", "", "DEVT EDID KSIZ KWDA OBND SMLS XALG", "ODTY"),
    ("SPCH", "", "DIFF EDID KSIZ KWDA SGEN SPLO SPMA SPQU SPWI SRAN", ""),
    ("SPEL", "DESC FULL", "ANAM CTDA EDID EFID EFIF EFIT ETYP KSIZ KWDA MNAM MUID OBND PDSH PUSH SPIT ZNAM", "ODTY"),
    ("SPGD", "", "DATA EDID MNAM", ""),
    ("STAG", "", "EDID STAD STAE STMS", ""),
    ("STAT", "FULL", "ANAM BFCB BFCE BNAM CNAM DEFL DNAM EDID FLCS FLLD FLTR FTYP INTV KSIZ KWDA MODL MOLM NVNM OBND ODTY OPDS PRPS PTCL PTT2 REFL SNBH SNTP STLS VLMS XALG XFLG", ""),
    ("STBH", "", "BFCB BFCE CITC CTDA EDID ENAM FLTR INTV PNAM UNAM VNAM", ""),
    ("STDT", "FULL", "ANAM BFCB BFCE BNAM DATA DNAM EDID ENAM FLLD KSIZ KWDA MODL PCCC PNAM XMPM", ""),
    ("STMP", "", "BFCB BFCE BNAM CNAM EDID ENAM FLTR INAM INTV ONAM PNAM RNAM SNAM TNAM", "STPT"),
    ("STND", "", "ANAM BFCB BFCE CNAM EDID FLLD FLTR FLTV FNAM INTV KSIZ KWDA MODL NNAM OBND", "ODTY SNST"),
    ("SUNP", "", "EDID RDIF REFL RFDP", ""),
    ("TERM", "FULL", "ANAM BFCB BFCE BNAM CNAM DNAM EDID FLLD FNAM FNPR FTMP FTYP GNAM JNAM KSIZ KWDA MNAM MODL MOLM NAM1 OBND ODTY PNAM PRPS PTT2 REFL SNAM SNTP STOP VMAD WBDT XFLG XMRK", ""),
    ("TMLM", "BTXT FULL INAM ISTX ITXT UNAM", "BNAM BSIZ CIS1 CIS2 CTDA DNAM EDID ISET ISIZ ITID SNAM TMVT TNAM TPLT VMAD XLOC", ""),
    ("TODD", "", "BFCB BFCE EDID KSIZ KWDA REFL", ""),
    ("TRAV", "", "DNAM EDID", ""),
    ("TRNS", "", "BNAM DATA EDID ENAM", ""),
    ("TXST", "", "DNAM DODT EDID MNAM OBND TX00 TX01 TX08 TX09 TX17 TX19", "ODTY"),
    ("VOLI", "", "EDID REFL", ""),
    ("VTYP", "", "ANAM DNAM EDID PNAM", ""),
    ("WATR", "FULL", "ANAM DATA DNAM EDID ENAM FNAM GNAM HNAM JNAM LNAM MNAM NAM0 NAM1 QNAM UNAM WASH XNAM YNAM", ""),
    ("WBAR", "", "EDID ZNAM", ""),
    ("WEAP", "FULL WABB", "ANAM APPR BAMT BFCB BFCE BIDS BNAM DAMA EAMT EDID EITM ENAM ETYP FLLD INRD KSIZ KWDA MOD4 MODL NNAM OBND OBTE OBTF OBTS OPDS PDSH PTCL PTT2 PUSH QNAM QUPA REFL STCP STOP WAIM WAM2 WAUD WCHG WDMG WFIR WFLG WGEN WMEL WRLO WTRM WTUR WVAR WVIS XALG", "DESC MO4S ODTY"),
    ("WKMF", "", "EDID WMKA WMSD WMSI WMSS WMTI", ""),
    ("WRLD", "FULL", "BFCB BFCE BNAM CITC CLSZ CNAM CTDA DATA DNAM EDID FNAM GNAM HNAM LNAM MNAM NAM0 NAM2 NAM3 NAM4 NAM7 NAM8 NAM9 NAMA OFST ONAM PNAM RNAM SNAM VNAM WAMB WHGT WNAM XCLW XEMP XLCN XNAM XWEM YNAM ZNAM", "NAM1 NAM5 NAM6 UNAM"),
    ("WTHR", "", "DALC DATA EDID FLLD FNAM GNAM HNAM IMSP JNAM KSIZ KWDA LNAM MNAM MODL NAM0 NAM1 NAM4 NNAM PNAM QNAM RNAM TNAM UNAM VNAM WNAM", "CLDC"),
    ("WTHS", "", "EDID RDIF REFL RFDP", ""),
    ("WWED", "", "CNAM EDID WSED WTED", ""),
    ("ZOOM", "", "EDID ZNAM", ""),
];

fn row(
    record: [u8; 4],
) -> Option<&'static (&'static str, &'static str, &'static str, &'static str)> {
    TABLE.iter().find(|(r, ..)| r.as_bytes() == record)
}

fn listed(list: &str, sub: [u8; 4]) -> bool {
    list.split(' ').any(|s| s.as_bytes() == sub)
}

/// What the corpus says about subrecord `sub` in records of type `record`.
pub fn class(record: [u8; 4], sub: [u8; 4]) -> Class {
    match row(record) {
        Some((_, l, _, _)) if listed(l, sub) => Class::LString,
        Some((_, _, n, _)) if listed(n, sub) => Class::NotLString,
        _ => Class::Unclassified,
    }
}

/// The limitation reported when a subrecord is treated as an lstring by this table.
pub fn table_note(record: [u8; 4], sub: [u8; 4]) -> String {
    format!(
        "{}.{} treated as an lstring by the corpus lstring table (edit::lstrings; corpus): the schema has no member for it here (a block it keeps raw, or a record type it does not define)",
        String::from_utf8_lossy(&record),
        String::from_utf8_lossy(&sub)
    )
}

/// Every lstring the schema defines, per `(record type, subrecord signature)`: the member
/// paths inside the subrecord (`""` for a whole-subrecord lstring, `.Name` or
/// `.Entries[].Text` for one inside a struct, an array or a union variant). Built from the
/// definitions alone (no corpus), so it is what `set`, `build` and `validate` read
/// lstrings from (M3.E4).
pub fn schema_lstrings(
    schema: &kitkraft_schema::Schema,
) -> std::collections::BTreeMap<([u8; 4], [u8; 4]), Vec<String>> {
    use kitkraft_schema::FieldType;
    fn walk(ty: &FieldType, at: &str, out: &mut Vec<String>, depth: usize) {
        if depth > 32 {
            return;
        }
        let next = depth.saturating_add(1);
        match ty {
            FieldType::LString => out.push(at.to_string()),
            FieldType::Ref { ty, .. } => walk(ty, at, out, next),
            FieldType::Array { element, .. } => walk(element, &format!("{at}[]"), out, next),
            FieldType::Struct { fields } => {
                for f in fields {
                    walk(&f.ty, &format!("{at}.{}", f.name), out, next);
                }
            }
            FieldType::Union { decider, variants } => {
                let named = matches!(decider, kitkraft_schema::Decider::Field { .. });
                for v in variants {
                    let here = if named {
                        format!("{at}.{}", v.name)
                    } else {
                        at.to_string()
                    };
                    walk(&v.ty, &here, out, next);
                }
            }
            _ => {}
        }
    }
    let mut out: std::collections::BTreeMap<([u8; 4], [u8; 4]), Vec<String>> =
        std::collections::BTreeMap::new();
    for def in schema.records() {
        let record = def.signature.as_bytes();
        for (_, sub) in def.leaf_subrecords() {
            let mut paths = Vec::new();
            walk(&sub.ty, "", &mut paths, 0);
            if !paths.is_empty() {
                let entry = out.entry((record, sub.signature.as_bytes())).or_default();
                for p in paths {
                    if !entry.contains(&p) {
                        entry.push(p);
                    }
                }
            }
        }
    }
    out
}

/// How the corpus table and the schema compare (M3.E4's cross-check).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Comparison {
    /// Table lstrings the schema types as whole-subrecord lstrings: the schema is used.
    pub agree: Vec<String>,
    /// Table lstrings the schema has no member for, because the record type keeps them
    /// inside a block it stores raw (`any_until`, the base form component blocks): the
    /// table is the fallback there, with a limitation ([`table_note`]).
    pub fallback: Vec<String>,
    /// Conflicts: a table lstring the schema defines otherwise (or whose record type it
    /// does not define), or a pair the table measured as "not an lstring" that the schema
    /// types as one. A conflict is a question for lane B; the schema wins meanwhile.
    pub conflicts: Vec<String>,
}

/// Whether a member list (or a group in it) keeps a block of subrecords raw (`any_until`).
fn has_raw_block(members: &[kitkraft_schema::Member], depth: usize) -> bool {
    depth <= 16
        && members.iter().any(|m| match m {
            kitkraft_schema::Member::AnyUntil(_) => true,
            kitkraft_schema::Member::Group(g) => has_raw_block(&g.members, depth.saturating_add(1)),
            kitkraft_schema::Member::Subrecord(_) => false,
        })
}

/// Compares the corpus [`TABLE`] with the schema's lstrings ([`schema_lstrings`]).
pub fn compare(schema: &kitkraft_schema::Schema) -> Comparison {
    let typed = schema_lstrings(schema);
    let mut out = Comparison::default();
    for (record, lstrings, others, _) in TABLE {
        let Ok(r) = <[u8; 4]>::try_from(record.as_bytes()) else {
            continue;
        };
        let def = kitkraft_schema::Signature::from_bytes(r).and_then(|sig| schema.record(sig));
        let raw_block = def.is_some_and(|d| has_raw_block(&d.members, 0));
        for (list, table_says) in [(lstrings, true), (others, false)] {
            for sub in list.split(' ').filter(|s| !s.is_empty()) {
                let Ok(s) = <[u8; 4]>::try_from(sub.as_bytes()) else {
                    continue;
                };
                let paths = typed.get(&(r, s));
                let whole = paths.is_some_and(|p| p.iter().any(String::is_empty));
                let pair = format!("{record}.{sub}");
                let defined_here = def.is_some_and(|d| {
                    d.leaf_subrecords()
                        .iter()
                        .any(|(_, m)| m.signature.as_bytes() == s)
                });
                match (table_says, whole) {
                    (true, true) => out.agree.push(pair),
                    (true, false) if raw_block && !defined_here => out.fallback.push(pair),
                    (true, false) => out.conflicts.push(format!(
                        "{pair}: an lstring in the corpus table, {}",
                        if def.is_none() {
                            "and the schema does not define the record type"
                        } else {
                            "not a whole-subrecord lstring in the schema"
                        }
                    )),
                    (false, true) => out.conflicts.push(format!(
                        "{pair}: not an lstring in the corpus table, an lstring in the schema"
                    )),
                    (false, false) => {}
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// M3.E4: the corpus table is a cross-check of the schema now that lane B typed its
    /// 105 lstring signatures (M3.B8). 100 are whole-subrecord schema lstrings; 5 sit in
    /// the base form component blocks the schema keeps raw (CHAL's activities, GBFM's hull
    /// name, the PNDT and STDT names), where the table stays the fallback; nothing the
    /// table measured as "not an lstring" is one in the schema.
    #[test]
    fn the_schema_types_every_table_lstring() {
        let schema = kitkraft_schema::builtin().unwrap();
        let c = compare(schema);
        assert_eq!(c.conflicts, Vec::<String>::new());
        assert_eq!(
            c.fallback,
            [
                "CHAL.DESC",
                "CHAL.FULL",
                "GBFM.HULL",
                "PNDT.FULL",
                "STDT.FULL"
            ]
        );
        assert_eq!(c.agree.len(), 100);
        let lstrings: usize = TABLE
            .iter()
            .map(|(_, l, ..)| l.split(' ').filter(|s| !s.is_empty()).count())
            .sum();
        assert_eq!(lstrings, c.agree.len() + c.fallback.len());
    }

    /// Where the schema has the member, a layout comes from it with no table limitation;
    /// inside a raw component block the table is the fallback and says so.
    #[test]
    fn schema_first_table_as_fallback() {
        use crate::edit::fields::layouts;
        let acti = layouts(*b"ACTI", &[*b"EDID", *b"ATTX"]);
        let attx = acti.get(1).cloned().flatten().unwrap();
        assert_eq!(attx.ty, kitkraft_schema::FieldType::LString);
        assert_eq!(attx.fallback, None);
        let gbfm = layouts(*b"GBFM", &[*b"EDID", *b"BFCB", *b"HULL", *b"BFCE"]);
        let hull = gbfm.get(2).cloned().flatten().unwrap();
        assert_eq!(hull.ty, kitkraft_schema::FieldType::LString);
        let note = hull.fallback.unwrap();
        assert!(
            note.starts_with("GBFM.HULL treated as an lstring by the corpus lstring table"),
            "{note}"
        );
    }

    /// `distinct` values, `resolved` of them in one kind of table, `anchored` shared with
    /// another record type's `FULL`/`DESC`.
    fn counts(four_bytes: bool, distinct: u64, resolved: u64, anchored: u64) -> Counts {
        Counts {
            occurrences: 100,
            four_bytes: if four_bytes { 100 } else { 99 },
            nonzero: distinct,
            distinct,
            resolved,
            kinds: [resolved, 0, 0],
            form_ids: 0,
            anchored,
        }
    }

    const X: [u8; 4] = *b"ATTX";

    #[test]
    fn thresholds() {
        assert_eq!(classify(X, &counts(true, 10, 10, 0)), Class::LString);
        assert_eq!(classify(X, &counts(true, 100, 96, 1)), Class::LString);
        // Too few values, too few resolving, shared with FULL/DESC IDs, not 4 bytes.
        assert_eq!(classify(X, &counts(true, 4, 4, 0)), Class::Unclassified);
        assert_eq!(classify(X, &counts(true, 100, 95, 0)), Class::Unclassified);
        assert_eq!(classify(X, &counts(true, 100, 100, 2)), Class::Unclassified);
        assert_eq!(
            classify(*b"FULL", &counts(true, 100, 100, 100)),
            Class::LString
        );
        assert_eq!(classify(X, &counts(true, 100, 50, 0)), Class::NotLString);
        assert_eq!(classify(X, &counts(false, 100, 100, 0)), Class::NotLString);
        assert_eq!(classify(X, &counts(true, 0, 0, 0)), Class::Unclassified);
        // Spread over the three kinds of table: coincidence.
        let mut spread = counts(true, 100, 100, 0);
        spread.kinds = [40, 30, 30];
        assert_eq!(classify(X, &spread), Class::NotLString);
    }
}
