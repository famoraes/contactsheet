//! Descriptive metadata embedded in exported files.
//!
//! Everything goes into one XMP packet, which photo management apps read first,
//! using the standard schemas (Dublin Core, IPTC Core, EXIF) that they map to
//! their filterable fields. A few plain TIFF tags are added for
//! readers that only look at EXIF/TIFF.
//!
//! Film has no standard fields for stock, format, process or archive location, so
//! they become hierarchical keywords (`Film|Stock|Kodak Portra 400`,
//! `Archive|Binder 3|Page 12`) that every DAM can filter on, plus copies in our
//! own `cs:` namespace.

use serde::Deserialize;

pub const SOFTWARE: &str = "ContactSheet";
const CS_NS: &str = "https://contactsheet.app/ns/1.0/";

/// One level of the physical archive location, e.g. `{ name: "Binder", value: "3" }`.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ArchiveLevel {
    pub name: String,
    pub value: String,
}

/// User metadata for one frame. Empty strings mean "not set".
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Metadata {
    pub film_stock: String,
    pub format: String,
    pub process: String,
    pub camera: String,
    pub lens: String,
    pub iso: Option<u32>,
    /// `YYYY`, `YYYY-MM` or `YYYY-MM-DD`; anything else is ignored.
    pub date_shot: String,
    pub roll_id: String,
    pub description: String,
    pub location: String,
    pub city: String,
    pub country: String,
    pub archive: Vec<ArchiveLevel>,
    /// Extra keywords; `|` separates hierarchy levels.
    pub keywords: Vec<String>,
    pub creator: String,
    pub copyright: String,
}

/// Provenance written alongside the user metadata.
#[derive(Clone, Debug, Default)]
pub struct Provenance {
    /// Path of the source negative scan.
    pub source: String,
    /// Conversion settings (JSON), so a positive can be traced back and re-made.
    pub settings_json: String,
}

/// What to embed in an exported file.
#[derive(Clone, Debug, Default)]
pub struct Embed {
    pub xmp: String,
    /// Description, camera, creator, copyright as ASCII for TIFF/EXIF-only readers.
    pub image_description: Option<String>,
    pub model: Option<String>,
    pub artist: Option<String>,
    pub copyright: Option<String>,
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            // Control characters other than tab/newline are not allowed in XML 1.0.
            c if (c as u32) < 0x20 && c != '\t' && c != '\n' && c != '\r' => {}
            c => out.push(c),
        }
    }
    out
}

/// Value for a TIFF ASCII tag. Only pure-ASCII text is written: readers reconcile
/// these tags with the XMP and may prefer them, so a lossy copy (e.g. "(c)" for
/// "©") would override the real value. Non-ASCII text lives only in the XMP.
fn ascii(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty() && t.bytes().all(|b| (0x20..0x7f).contains(&b))).then(|| t.to_string())
}

/// Accepts `YYYY`, `YYYY-MM`, `YYYY-MM-DD` with plausible ranges.
pub fn valid_xmp_date(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    let num = |p: &str, len: usize, lo: u32, hi: u32| {
        p.len() == len && p.bytes().all(|b| b.is_ascii_digit()) && (lo..=hi).contains(&p.parse::<u32>().unwrap_or(0))
    };
    match parts.as_slice() {
        [y] => num(y, 4, 1800, 2999),
        [y, m] => num(y, 4, 1800, 2999) && num(m, 2, 1, 12),
        [y, m, d] => num(y, 4, 1800, 2999) && num(m, 2, 1, 12) && num(d, 2, 1, 31),
        _ => false,
    }
}

/// Remove duplicates, keeping the first occurrence.
fn uniq(v: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    v.into_iter().filter(|s| seen.insert(s.clone())).collect()
}

fn clean(s: &str) -> Option<&str> {
    let t = s.trim();
    (!t.is_empty()).then_some(t)
}

/// Hierarchical keyword paths (levels joined by `|`).
fn keyword_paths(m: &Metadata) -> Vec<String> {
    let mut paths = Vec::new();
    // `|` is the hierarchy separator, so it can't appear inside a level.
    let level = |s: &str| s.trim().replace('|', "/");
    for (group, value) in [("Stock", &m.film_stock), ("Format", &m.format), ("Process", &m.process)] {
        if let Some(v) = clean(value) {
            paths.push(format!("Film|{group}|{}", level(v)));
        }
    }
    let archive: Vec<String> = m
        .archive
        .iter()
        .filter_map(|a| clean(&a.value).map(|v| level(&format!("{} {}", a.name.trim(), v).trim().to_string())))
        .collect();
    if !archive.is_empty() {
        paths.push(format!("Archive|{}", archive.join("|")));
    }
    for k in &m.keywords {
        let parts: Vec<&str> = k.split('|').map(str::trim).filter(|p| !p.is_empty()).collect();
        if !parts.is_empty() {
            paths.push(parts.join("|"));
        }
    }
    uniq(paths)
}

/// Human-readable archive location, e.g. "Binder 3 / Page 12 / Strip 4".
pub fn archive_label(m: &Metadata) -> Option<String> {
    let parts: Vec<String> = m
        .archive
        .iter()
        .filter_map(|a| clean(&a.value).map(|v| format!("{} {}", a.name.trim(), v).trim().to_string()))
        .collect();
    (!parts.is_empty()).then(|| parts.join(" / "))
}

struct Xmp(String);

impl Xmp {
    fn text(&mut self, prop: &str, value: Option<&str>) {
        if let Some(v) = value.and_then(clean) {
            self.0.push_str(&format!("   <{prop}>{}</{prop}>\n", esc(v)));
        }
    }
    fn array(&mut self, prop: &str, kind: &str, items: &[String]) {
        if items.is_empty() {
            return;
        }
        self.0.push_str(&format!("   <{prop}>\n    <rdf:{kind}>\n"));
        for it in items {
            self.0.push_str(&format!("     <rdf:li>{}</rdf:li>\n", esc(it)));
        }
        self.0.push_str(&format!("    </rdf:{kind}>\n   </{prop}>\n"));
    }
    fn lang_alt(&mut self, prop: &str, value: Option<&str>) {
        if let Some(v) = value.and_then(clean) {
            self.0.push_str(&format!(
                "   <{prop}>\n    <rdf:Alt>\n     <rdf:li xml:lang=\"x-default\">{}</rdf:li>\n    </rdf:Alt>\n   </{prop}>\n",
                esc(v)
            ));
        }
    }
}

/// Build the XMP packet for a frame.
pub fn build_xmp(m: &Metadata, prov: &Provenance) -> String {
    let mut x = Xmp(String::new());
    x.text("xmp:CreatorTool", Some(SOFTWARE));

    // Descriptive (IPTC Core / Dublin Core).
    x.lang_alt("dc:description", Some(&m.description));
    if let Some(c) = clean(&m.creator) {
        x.array("dc:creator", "Seq", &[c.to_string()]);
    }
    if let Some(c) = clean(&m.copyright) {
        x.lang_alt("dc:rights", Some(c));
        x.text("xmpRights:Marked", Some("True"));
    }
    x.text("photoshop:TransmissionReference", Some(&m.roll_id)); // IPTC Job Identifier
    x.text("Iptc4xmpCore:Location", Some(&m.location)); // IPTC Sublocation
    x.text("photoshop:City", Some(&m.city));
    x.text("photoshop:Country", Some(&m.country));
    let archive = archive_label(m);
    x.text("photoshop:Instructions", archive.as_deref().map(|a| format!("Archive: {a}")).as_deref());

    // Keywords: leaf terms in dc:subject, full paths in lr:hierarchicalSubject.
    let paths = keyword_paths(m);
    let leaves = uniq(paths.iter().filter_map(|p| p.rsplit('|').next().map(str::to_string)).collect());
    x.array("dc:subject", "Bag", &leaves);
    x.array("lr:hierarchicalSubject", "Bag", &paths);

    // Capture details (EXIF in XMP).
    if valid_xmp_date(m.date_shot.trim()) {
        x.text("exif:DateTimeOriginal", Some(m.date_shot.trim()));
        x.text("photoshop:DateCreated", Some(m.date_shot.trim()));
    }
    x.text("tiff:Model", Some(&m.camera));
    x.text("exifEX:LensModel", Some(&m.lens));
    x.text("aux:Lens", Some(&m.lens));
    if let Some(iso) = m.iso.filter(|&i| i > 0) {
        x.array("exif:ISOSpeedRatings", "Seq", &[iso.to_string()]);
        x.text("exifEX:PhotographicSensitivity", Some(&iso.to_string()));
    }

    // Provenance.
    if let Some(src) = clean(&prov.source) {
        x.0.push_str(&format!(
            "   <xmpMM:DerivedFrom rdf:parseType=\"Resource\">\n    <stRef:filePath>{}</stRef:filePath>\n   </xmpMM:DerivedFrom>\n",
            esc(src)
        ));
        x.text("cs:SourceFile", Some(src));
    }
    x.text("cs:FilmStock", Some(&m.film_stock));
    x.text("cs:FilmFormat", Some(&m.format));
    x.text("cs:Process", Some(&m.process));
    x.text("cs:RollId", Some(&m.roll_id));
    x.text("cs:ArchiveLocation", archive.as_deref());
    x.text("cs:Settings", Some(&prov.settings_json));

    format!(
        "<?xpacket begin=\"\u{FEFF}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n\
<x:xmpmeta xmlns:x=\"adobe:ns:meta/\" x:xmptk=\"{SOFTWARE}\">\n\
 <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n\
  <rdf:Description rdf:about=\"\"\n\
    xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"\n\
    xmlns:xmpMM=\"http://ns.adobe.com/xap/1.0/mm/\"\n\
    xmlns:stRef=\"http://ns.adobe.com/xap/1.0/sType/ResourceRef#\"\n\
    xmlns:xmpRights=\"http://ns.adobe.com/xap/1.0/rights/\"\n\
    xmlns:dc=\"http://purl.org/dc/elements/1.1/\"\n\
    xmlns:photoshop=\"http://ns.adobe.com/photoshop/1.0/\"\n\
    xmlns:Iptc4xmpCore=\"http://iptc.org/std/Iptc4xmpCore/1.0/xmlns/\"\n\
    xmlns:lr=\"http://ns.adobe.com/lightroom/1.0/\"\n\
    xmlns:tiff=\"http://ns.adobe.com/tiff/1.0/\"\n\
    xmlns:exif=\"http://ns.adobe.com/exif/1.0/\"\n\
    xmlns:exifEX=\"http://cipa.jp/exif/1.0/\"\n\
    xmlns:aux=\"http://ns.adobe.com/exif/1.0/aux/\"\n\
    xmlns:cs=\"{CS_NS}\">\n\
{}  </rdf:Description>\n\
 </rdf:RDF>\n\
</x:xmpmeta>\n\
<?xpacket end=\"w\"?>",
        x.0
    )
}

/// Everything to embed for one exported frame.
pub fn embed(m: &Metadata, prov: &Provenance) -> Embed {
    Embed {
        xmp: build_xmp(m, prov),
        image_description: ascii(&m.description),
        model: ascii(&m.camera),
        artist: ascii(&m.creator),
        copyright: ascii(&m.copyright),
    }
}

/// Insert an XMP APP1 segment into a baseline JPEG, after SOI and any JFIF APP0.
pub fn jpeg_with_xmp(jpeg: &[u8], xmp: &str) -> Option<Vec<u8>> {
    const HEADER: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
    let len = 2 + HEADER.len() + xmp.len();
    if jpeg.len() < 4 || jpeg[0..2] != [0xFF, 0xD8] || len > u16::MAX as usize {
        return None;
    }
    let mut at = 2;
    if jpeg.len() >= 6 && jpeg[2..4] == [0xFF, 0xE0] {
        at = 4 + u16::from_be_bytes([jpeg[4], jpeg[5]]) as usize;
        if at > jpeg.len() {
            return None;
        }
    }
    let mut out = Vec::with_capacity(jpeg.len() + len + 2);
    out.extend_from_slice(&jpeg[..at]);
    out.extend_from_slice(&[0xFF, 0xE1]);
    out.extend_from_slice(&(len as u16).to_be_bytes());
    out.extend_from_slice(HEADER);
    out.extend_from_slice(xmp.as_bytes());
    out.extend_from_slice(&jpeg[at..]);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Metadata {
        Metadata {
            film_stock: "Kodak Portra 400".into(),
            format: "120 6×7".into(),
            process: "C-41".into(),
            camera: "Mamiya 7".into(),
            iso: Some(400),
            date_shot: "2026-03".into(),
            roll_id: "2026-014".into(),
            description: "Beach <dusk> & fog".into(),
            city: "São Paulo".into(),
            archive: vec![
                ArchiveLevel { name: "Binder".into(), value: "3".into() },
                ArchiveLevel { name: "Page".into(), value: "12".into() },
                ArchiveLevel { name: "Strip".into(), value: "".into() },
            ],
            keywords: vec!["Travel|Brazil".into(), " ".into()],
            copyright: "© 2026 Someone".into(),
            ..Default::default()
        }
    }

    #[test]
    fn xmp_has_filterable_fields() {
        let x = build_xmp(&sample(), &Provenance { source: "/scans/a.tif".into(), settings_json: "{\"a\":1}".into() });
        for needle in [
            "<rdf:li>Film|Stock|Kodak Portra 400</rdf:li>",
            "<rdf:li>Film|Format|120 6×7</rdf:li>",
            "<rdf:li>Archive|Binder 3|Page 12</rdf:li>",
            "<rdf:li>Travel|Brazil</rdf:li>",
            "<rdf:li>Kodak Portra 400</rdf:li>",
            "<photoshop:TransmissionReference>2026-014</photoshop:TransmissionReference>",
            "<exif:DateTimeOriginal>2026-03</exif:DateTimeOriginal>",
            "<tiff:Model>Mamiya 7</tiff:Model>",
            "<rdf:li>400</rdf:li>",
            "Beach &lt;dusk&gt; &amp; fog",
            "<photoshop:City>São Paulo</photoshop:City>",
            "<photoshop:Instructions>Archive: Binder 3 / Page 12</photoshop:Instructions>",
            "<stRef:filePath>/scans/a.tif</stRef:filePath>",
            "<cs:Settings>{&quot;a&quot;:1}</cs:Settings>",
            "<xmpRights:Marked>True</xmpRights:Marked>",
        ] {
            assert!(x.contains(needle), "missing {needle}\n{x}");
        }
        assert!(!x.contains("exifEX:LensModel"), "empty fields are omitted");
    }

    #[test]
    fn dates() {
        assert!(valid_xmp_date("1998"));
        assert!(valid_xmp_date("1998-07"));
        assert!(valid_xmp_date("1998-07-31"));
        for bad in ["98", "1998-7", "1998-13", "1998-07-32", "July 1998", ""] {
            assert!(!valid_xmp_date(bad), "{bad}");
        }
    }

    #[test]
    fn ascii_tags() {
        let e = embed(&sample(), &Provenance::default());
        assert_eq!(e.copyright, None, "non-ASCII stays XMP-only");
        assert_eq!(e.model.as_deref(), Some("Mamiya 7"));
        assert_eq!(e.artist, None);
    }

    #[test]
    fn jpeg_insertion_after_jfif() {
        let jpeg = [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0xAA, 0xBB, 0xFF, 0xD9];
        let out = jpeg_with_xmp(&jpeg, "<x/>").unwrap();
        assert_eq!(&out[..8], &jpeg[..8]);
        assert_eq!(&out[8..10], &[0xFF, 0xE1]);
        let len = u16::from_be_bytes([out[10], out[11]]) as usize;
        assert_eq!(len, 2 + 29 + 4);
        assert_eq!(&out[10 + len..], &[0xFF, 0xD9]);
    }
}
