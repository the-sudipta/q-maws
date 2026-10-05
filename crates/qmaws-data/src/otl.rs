//! Taxonomic groups from the Open Tree of Life (plan 4.8.1 and 6.7) for
//! the group bands of the Halo Tree.
//!
//! Endpoints (API v3, checked against the documentation and a live call on
//! 2026-10-03, see docs/EXTERNAL_TOOLS.md):
//!
//! - `POST /v3/tnrs/match_names` with `{"names": [...],
//!   "do_approximate_matching": false}`: `results[i].name` and
//!   `results[i].matches[j].taxon.ott_id`, plus `taxonomy.version`;
//! - `POST /v3/taxonomy/taxon_info` with `{"ott_id": n,
//!   "include_lineage": true}`: `lineage` lists the higher taxa (least
//!   inclusive first) with `rank` and `name`.
//!
//! Search names are the taxon names with underscores read as spaces;
//! nothing else is changed (strain suffixes are kept). A name with no
//! match, or more than one, is left without a group. The rank of the
//! groups is the most specific of genus, family, order, class and phylum
//! that gives between 2 and 7 groups. The record of the queries (API,
//! taxonomy version, date, names, SHA-256 of every response) is returned
//! for the audit folder.

use crate::download::HttpFetcher;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const API: &str = "https://api.opentreeoflife.org/v3";

/// Ranks tried for the groups, most specific first.
pub const RANKS: [&str; 5] = ["genus", "family", "order", "class", "phylum"];

/// Largest number of groups (the size of the categorical palette).
pub const MAX_GROUPS: usize = 7;

/// Sends a JSON body and returns the response text.
pub trait Poster {
    fn post_json(&self, url: &str, body: &str) -> Result<String, String>;
}

impl Poster for HttpFetcher {
    fn post_json(&self, url: &str, body: &str) -> Result<String, String> {
        HttpFetcher::post_json(self, url, body)
    }
}

/// Groups found and the record of the queries.
#[derive(Debug, Clone, PartialEq)]
pub struct TaxonomyGroups {
    /// Rank of the groups; `None` when no rank gives 2 to 7 groups.
    pub rank: Option<String>,
    /// Taxon (as in the run) to group name.
    pub groups: BTreeMap<String, String>,
    pub taxonomy_version: String,
    pub matched: usize,
    pub unmatched: Vec<String>,
    pub ambiguous: Vec<String>,
    /// For `audit/otl_taxonomy.json`.
    pub record: Value,
}

fn sha256(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The search name of a taxon name.
pub fn search_name(name: &str) -> String {
    name.replace('_', " ").trim().to_string()
}

/// Picks the most specific rank with 2 to [`MAX_GROUPS`] groups among the
/// taxa that have that rank in their lineage.
pub fn choose_rank(
    lineages: &BTreeMap<String, Vec<(String, String)>>,
) -> Option<(String, BTreeMap<String, String>)> {
    for rank in RANKS {
        let groups: BTreeMap<String, String> = lineages
            .iter()
            .filter_map(|(taxon, lin)| {
                lin.iter()
                    .find(|(r, _)| r == rank)
                    .map(|(_, name)| (taxon.clone(), name.clone()))
            })
            .collect();
        let mut distinct: Vec<&String> = groups.values().collect();
        distinct.sort();
        distinct.dedup();
        if (2..=MAX_GROUPS).contains(&distinct.len()) {
            return Some((rank.to_string(), groups));
        }
    }
    None
}

/// Looks up `taxa` (run name, search name) in the Open Tree of Life and
/// returns their groups. `date` is recorded (UTC, ISO 8601).
pub fn taxonomy_groups(
    poster: &dyn Poster,
    taxa: &[(String, String)],
    date: &str,
) -> Result<TaxonomyGroups, String> {
    let NameMatches {
        taxonomy_version: version,
        ott,
        unmatched,
        ambiguous,
        query,
        reply_sha256,
        ..
    } = match_taxa(poster, taxa)?;
    let ambiguous: Vec<String> = ambiguous.into_iter().map(|(t, _)| t).collect();
    let mut lineages: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    let mut info_hashes = Vec::new();
    for (taxon, id) in &ott {
        let body = json!({"ott_id": id, "include_lineage": true}).to_string();
        let text = poster.post_json(&format!("{API}/taxonomy/taxon_info"), &body)?;
        info_hashes.push(json!({"taxon": taxon, "ott_id": id, "sha256": sha256(&text)}));
        let v: Value =
            serde_json::from_str(&text).map_err(|e| format!("Open Tree of Life answer: {e}"))?;
        let lin = v["lineage"]
            .as_array()
            .map(|l| {
                l.iter()
                    .map(|x| {
                        (
                            x["rank"].as_str().unwrap_or("").to_string(),
                            x["name"].as_str().unwrap_or("").to_string(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        lineages.insert(taxon.clone(), lin);
    }
    let chosen = choose_rank(&lineages);
    let (rank, groups) = match chosen {
        Some((r, g)) => (Some(r), g),
        None => (None, BTreeMap::new()),
    };
    let record = json!({
        "api": API,
        "taxonomy_version": version,
        "date_utc": date,
        "match_names_query": query,
        "match_names_sha256": reply_sha256,
        "taxon_info": info_hashes,
        "matched": ott,
        "unmatched": unmatched,
        "ambiguous": ambiguous,
        "rank": rank,
        "groups": groups,
    });
    Ok(TaxonomyGroups {
        rank,
        groups,
        taxonomy_version: version,
        matched: ott.len(),
        unmatched,
        ambiguous,
        record,
    })
}

/// The canonical text of a parsed answer (keys sorted), so that the
/// recorded hash does not depend on spacing.
fn text_of(v: &Value) -> String {
    v.to_string()
}

/// Names matched exactly (no approximate matching) with
/// `POST /v3/tnrs/match_names`.
#[derive(Debug, Clone, PartialEq)]
pub struct NameMatches {
    pub taxonomy_version: String,
    /// Taxon (as in the run) to its one OTT id.
    pub ott: BTreeMap<String, u64>,
    /// Taxon to the matched taxonomy name.
    pub matched_names: BTreeMap<String, String>,
    pub unmatched: Vec<String>,
    /// Taxa with more than one match, with the candidates' names.
    pub ambiguous: Vec<(String, Vec<String>)>,
    pub query: Value,
    /// SHA-256 of the answer (canonical JSON text).
    pub reply_sha256: String,
}

/// Matches `taxa` (run name, search name) to OTT ids.
pub fn match_taxa(poster: &dyn Poster, taxa: &[(String, String)]) -> Result<NameMatches, String> {
    let names: Vec<&str> = taxa.iter().map(|(_, s)| s.as_str()).collect();
    let query = json!({"names": names, "do_approximate_matching": false});
    let url = format!("{API}/tnrs/match_names");
    let text = poster.post_json(&url, &query.to_string())?;
    let reply: Value =
        serde_json::from_str(&text).map_err(|e| format!("Open Tree of Life answer: {e}"))?;
    let taxonomy_version = reply["taxonomy"]["version"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();
    let results = reply["results"]
        .as_array()
        .ok_or("Open Tree of Life answer has no results")?;
    let by_search: BTreeMap<&str, &str> = taxa
        .iter()
        .map(|(run, s)| (s.as_str(), run.as_str()))
        .collect();
    let mut ott = BTreeMap::new();
    let mut matched_names = BTreeMap::new();
    let (mut unmatched, mut ambiguous) = (Vec::new(), Vec::new());
    let mut answered = std::collections::BTreeSet::new();
    for r in results {
        let name = r["name"].as_str().unwrap_or("");
        let Some(&run) = by_search.get(name) else {
            continue;
        };
        answered.insert(run);
        let matches = r["matches"].as_array().map(Vec::as_slice).unwrap_or(&[]);
        match matches {
            [] => unmatched.push(run.to_string()),
            [one] => match one["taxon"]["ott_id"].as_u64() {
                Some(id) => {
                    ott.insert(run.to_string(), id);
                    let n = one["taxon"]["unique_name"]
                        .as_str()
                        .or(one["taxon"]["name"].as_str())
                        .unwrap_or("");
                    matched_names.insert(run.to_string(), n.to_string());
                }
                None => unmatched.push(run.to_string()),
            },
            many => ambiguous.push((
                run.to_string(),
                many.iter()
                    .map(|m| {
                        m["taxon"]["unique_name"]
                            .as_str()
                            .or(m["taxon"]["name"].as_str())
                            .unwrap_or("?")
                            .to_string()
                    })
                    .collect(),
            )),
        }
    }
    // Names the service did not list in `results` (it lists them in
    // `unmatched_names`) have no match.
    for (run, _) in taxa {
        if !answered.contains(run.as_str()) {
            unmatched.push(run.clone());
        }
    }
    Ok(NameMatches {
        taxonomy_version,
        ott,
        matched_names,
        unmatched,
        ambiguous,
        query,
        reply_sha256: sha256(&text_of(&reply)),
    })
}

/// The search name without a strain or isolate suffix: the first two
/// words (genus and species). Used only when the user agrees (plan 6.7).
pub fn species_name(search: &str) -> String {
    search
        .split_whitespace()
        .take(2)
        .collect::<Vec<_>>()
        .join(" ")
}

/// The matching report for a reference tree: every taxon needs its own OTT
/// id, because the comparison needs the same taxa in both trees (plan 2.10).
#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceMatch {
    pub names: NameMatches,
    /// Taxa that matched the same OTT id (for example strains of one
    /// species); they cannot be told apart in the reference.
    pub same_taxon: Vec<(u64, Vec<String>)>,
}

impl ReferenceMatch {
    /// True when every one of `taxa` has its own OTT id.
    pub fn is_complete(&self, taxa: usize) -> bool {
        self.names.ott.len() == taxa && self.same_taxon.is_empty()
    }

    /// The report shown to the user.
    pub fn report(&self) -> Vec<String> {
        let mut lines = vec![format!(
            "Open Tree of Life taxonomy {}: {} taxa matched.",
            self.names.taxonomy_version,
            self.names.ott.len()
        )];
        for (taxon, name) in &self.names.matched_names {
            lines.push(format!(
                "  matched: {taxon} = {name} (ott{})",
                self.names.ott[taxon]
            ));
        }
        for taxon in &self.names.unmatched {
            lines.push(format!("  not found: {taxon}"));
        }
        for (taxon, candidates) in &self.names.ambiguous {
            lines.push(format!(
                "  more than one match: {taxon} (candidates: {})",
                candidates.join("; ")
            ));
        }
        for (id, taxa) in &self.same_taxon {
            lines.push(format!(
                "  the same taxon (ott{id}) for: {}",
                taxa.join(", ")
            ));
        }
        lines
    }
}

/// Matches `taxa` (run name, search name) for a reference tree.
pub fn reference_match(
    poster: &dyn Poster,
    taxa: &[(String, String)],
) -> Result<ReferenceMatch, String> {
    let names = match_taxa(poster, taxa)?;
    let mut by_id: BTreeMap<u64, Vec<String>> = BTreeMap::new();
    for (taxon, id) in &names.ott {
        by_id.entry(*id).or_default().push(taxon.clone());
    }
    let same_taxon = by_id.into_iter().filter(|(_, t)| t.len() > 1).collect();
    Ok(ReferenceMatch { names, same_taxon })
}

/// A reference tree from the Open Tree of Life synthetic tree.
#[derive(Debug, Clone, PartialEq)]
pub struct OtlReference {
    /// Newick with the run's taxon names as leaves; single-child nodes and
    /// internal labels removed. The synthetic tree may have polytomies.
    pub newick: String,
    pub synth_id: String,
    /// For `audit/reference.json`: API, synthetic tree, taxonomy, date,
    /// queries and SHA-256 of the answers.
    pub record: Value,
}

/// The tree induced on the synthetic tree by the matched taxa
/// (`POST /v3/tree_of_life/induced_subtree` with `label_format: "id"`).
/// `matched` must be complete (see [`ReferenceMatch::is_complete`]).
pub fn induced_reference(
    poster: &dyn Poster,
    matched: &ReferenceMatch,
    date: &str,
) -> Result<OtlReference, String> {
    let about_text = poster.post_json(&format!("{API}/tree_of_life/about"), "{}")?;
    let about: Value =
        serde_json::from_str(&about_text).map_err(|e| format!("Open Tree of Life answer: {e}"))?;
    let synth_id = about["synth_id"].as_str().unwrap_or("unknown").to_string();
    let ids: Vec<u64> = matched.names.ott.values().copied().collect();
    let query = json!({"ott_ids": ids, "label_format": "id"});
    let text = poster.post_json(
        &format!("{API}/tree_of_life/induced_subtree"),
        &query.to_string(),
    )?;
    let reply: Value =
        serde_json::from_str(&text).map_err(|e| format!("Open Tree of Life answer: {e}"))?;
    let raw = reply["newick"]
        .as_str()
        .ok_or("the Open Tree of Life answer has no tree")?;
    // Node label to taxon: `ott<id>`, or the node a "broken" taxon maps to.
    let mut label_to_taxon: BTreeMap<String, String> = BTreeMap::new();
    let mut broken = Vec::new();
    for (taxon, id) in &matched.names.ott {
        let key = format!("ott{id}");
        match reply["broken"][&key].as_str() {
            Some(node) => {
                broken.push(taxon.clone());
                label_to_taxon.insert(node.to_string(), taxon.clone());
            }
            None => {
                label_to_taxon.insert(key, taxon.clone());
            }
        }
    }
    let tree = qmaws_core::newick::Tree::parse(raw)
        .map_err(|e| format!("the Open Tree of Life tree cannot be read: {e}"))?;
    let newick = taxa_newick(&tree, &label_to_taxon)?;
    let record = json!({
        "api": API,
        "synth_id": synth_id,
        "synthetic_tree_date": about["date_created"],
        "taxonomy_version": about["taxonomy_version"],
        "date_utc": date,
        "match_names_query": matched.names.query,
        "match_names_sha256": matched.names.reply_sha256,
        "about_sha256": sha256(&text_of(&about)),
        "induced_subtree_query": query,
        "induced_subtree_sha256": sha256(&text_of(&reply)),
        "matched": matched.names.ott,
        "broken_taxa": broken,
    });
    Ok(OtlReference {
        newick,
        synth_id,
        record,
    })
}

/// Writes `tree` with leaves renamed by `labels` (every leaf must be in
/// it), without internal labels and with single-child nodes removed.
fn taxa_newick(
    tree: &qmaws_core::newick::Tree,
    labels: &BTreeMap<String, String>,
) -> Result<String, String> {
    fn write(
        t: &qmaws_core::newick::Tree,
        n: usize,
        labels: &BTreeMap<String, String>,
        seen: &mut Vec<String>,
    ) -> Result<String, String> {
        let node = &t.nodes[n];
        if node.children.is_empty() {
            let label = node.label.clone().unwrap_or_default();
            let taxon = labels
                .get(&label)
                .ok_or_else(|| format!("the tree has a tip {label} that is none of the taxa"))?;
            seen.push(taxon.clone());
            return Ok(taxon.clone());
        }
        if let Some(label) = &node.label {
            if let Some(taxon) = labels.get(label) {
                return Err(format!(
                    "{taxon} is an inner node of the synthetic tree (it contains another taxon of the run)"
                ));
            }
        }
        let parts: Vec<String> = node
            .children
            .iter()
            .map(|&c| write(t, c, labels, seen))
            .collect::<Result<_, _>>()?;
        Ok(if parts.len() == 1 {
            parts.into_iter().next().expect("one part")
        } else {
            format!("({})", parts.join(","))
        })
    }
    let mut seen = Vec::new();
    let mut text = write(tree, tree.root, labels, &mut seen)?;
    if !text.starts_with('(') {
        return Err("the induced tree has a single tip".into());
    }
    seen.sort();
    let mut expected: Vec<String> = labels.values().cloned().collect();
    expected.sort();
    if seen != expected {
        let missing: Vec<&String> = expected.iter().filter(|x| !seen.contains(x)).collect();
        return Err(format!(
            "the induced tree does not have every taxon once (missing: {missing:?})"
        ));
    }
    text.push(';');
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// Answers with recorded responses (shapes copied from the live API on
    /// 2026-10-03, shortened).
    struct Recorded {
        calls: RefCell<Vec<(String, String)>>,
    }

    impl Poster for Recorded {
        fn post_json(&self, url: &str, body: &str) -> Result<String, String> {
            self.calls
                .borrow_mut()
                .push((url.to_string(), body.to_string()));
            if url.ends_with("match_names") {
                return Ok(r#"{"results":[
                  {"name":"Astronotus ocellatus","matches":[{"taxon":{"ott_id":952936,"name":"Astronotus ocellatus"}}]},
                  {"name":"Oreochromis sp-KM2006","matches":[]},
                  {"name":"Amphiprion ocellaris","matches":[{"taxon":{"ott_id":2}}]},
                  {"name":"Halichoeres melanurus","matches":[{"taxon":{"ott_id":3}},{"taxon":{"ott_id":4}}]}],
                  "taxonomy":{"version":"3.7"}}"#
                    .into());
            }
            let v: Value = serde_json::from_str(body).unwrap();
            let lineage = match v["ott_id"].as_u64() {
                Some(952936) => {
                    r#"[{"rank":"genus","name":"Astronotus"},{"rank":"family","name":"Cichlidae"},{"rank":"order","name":"Cichliformes"}]"#
                }
                _ => {
                    r#"[{"rank":"genus","name":"Amphiprion"},{"rank":"family","name":"Pomacentridae"},{"rank":"order","name":"Ovalentaria incertae sedis"}]"#
                }
            };
            Ok(format!(
                r#"{{"ott_id":{},"lineage":{lineage}}}"#,
                v["ott_id"]
            ))
        }
    }

    #[test]
    fn groups_come_from_the_lineages_and_the_queries_are_recorded() {
        let p = Recorded {
            calls: RefCell::new(Vec::new()),
        };
        let taxa: Vec<(String, String)> = [
            "Astronotus_ocellatus",
            "Oreochromis_sp-KM2006",
            "Amphiprion_ocellaris",
            "Halichoeres_melanurus",
        ]
        .iter()
        .map(|n| (n.to_string(), search_name(n)))
        .collect();
        let g = taxonomy_groups(&p, &taxa, "2026-10-03T00:00:00Z").unwrap();
        assert_eq!(g.taxonomy_version, "3.7");
        assert_eq!(g.matched, 2);
        assert_eq!(g.unmatched, vec!["Oreochromis_sp-KM2006"]);
        assert_eq!(g.ambiguous, vec!["Halichoeres_melanurus"]);
        // Genus gives 2 groups already.
        assert_eq!(g.rank.as_deref(), Some("genus"));
        assert_eq!(g.groups["Astronotus_ocellatus"], "Astronotus");
        assert_eq!(p.calls.borrow().len(), 3);
        assert!(p.calls.borrow()[0]
            .1
            .contains("\"do_approximate_matching\":false"));
        assert_eq!(g.record["taxonomy_version"], "3.7");
        assert_eq!(g.record["taxon_info"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn rank_with_two_to_seven_groups_is_chosen() {
        let lin = |pairs: &[(&str, &str)]| -> Vec<(String, String)> {
            pairs
                .iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect()
        };
        let mut l = BTreeMap::new();
        for i in 0..10 {
            l.insert(
                format!("t{i}"),
                lin(&[
                    ("genus", &format!("G{i}")),
                    ("family", if i < 5 { "F1" } else { "F2" }),
                ]),
            );
        }
        let (rank, groups) = choose_rank(&l).unwrap();
        assert_eq!(rank, "family");
        assert_eq!(groups.len(), 10);
        let mut one = BTreeMap::new();
        one.insert("a".to_string(), lin(&[("family", "F")]));
        one.insert("b".to_string(), lin(&[("family", "F")]));
        assert!(choose_rank(&one).is_none());
    }

    /// Answers for a reference tree, shaped like the live API on 2026-10-05
    /// (synthetic tree opentree16.1): single-child chains, `mrca` labels,
    /// and one "broken" taxon placed at an `mrca` node.
    struct ReferenceAnswers;

    impl Poster for ReferenceAnswers {
        fn post_json(&self, url: &str, _body: &str) -> Result<String, String> {
            Ok(if url.ends_with("match_names") {
                r#"{"results":[
                  {"name":"Homo sapiens","matches":[{"taxon":{"ott_id":770315,"unique_name":"Homo sapiens"}}]},
                  {"name":"Pan troglodytes","matches":[{"taxon":{"ott_id":417950,"unique_name":"Pan troglodytes"}}]},
                  {"name":"Gorilla gorilla","matches":[{"taxon":{"ott_id":417965,"unique_name":"Gorilla gorilla"}}]},
                  {"name":"Pongo abelii","matches":[{"taxon":{"ott_id":770295,"unique_name":"Pongo abelii"}}]},
                  {"name":"Mus musculus","matches":[{"taxon":{"ott_id":542509,"unique_name":"Mus musculus"}}]}],
                  "unmatched_names":[],"taxonomy":{"version":"3.7draft3"}}"#
                    .into()
            } else if url.ends_with("about") {
                r#"{"synth_id":"opentree16.1","date_created":"2025-12-20 00:55:58","taxonomy_version":"3.7draft3"}"#.into()
            } else {
                r#"{"broken":{"ott770295":"mrcaott770295ott3607692"},
                   "newick":"((((ott542509)mrcaott102ott321218)ott816256)mrcaott42ott30082,(((ott770315)mrcaott83926ott3607676,(ott417950)ott417957)mrcaott83926ott84217,(ott417965)ott417969,((mrcaott770295ott3607692)ott1082538)mrcaott770295ott3607719)mrcaott786ott83926)ott6520;"}"#
                    .into()
            })
        }
    }

    fn primates() -> Vec<(String, String)> {
        [
            "Homo_sapiens",
            "Pan_troglodytes",
            "Gorilla_gorilla",
            "Pongo_abelii",
            "Mus_musculus",
        ]
        .iter()
        .map(|n| (n.to_string(), search_name(n)))
        .collect()
    }

    #[test]
    fn the_induced_tree_has_the_taxon_names_and_no_single_child_nodes() {
        let m = reference_match(&ReferenceAnswers, &primates()).unwrap();
        assert!(m.is_complete(5), "{:?}", m.report());
        let r = induced_reference(&ReferenceAnswers, &m, "2026-10-05T00:00:00Z").unwrap();
        assert_eq!(r.synth_id, "opentree16.1");
        assert_eq!(
            r.newick,
            "(Mus_musculus,((Homo_sapiens,Pan_troglodytes),Gorilla_gorilla,Pongo_abelii));"
        );
        let t = qmaws_core::newick::Tree::parse(&r.newick).unwrap();
        assert_eq!(t.leaf_names().len(), 5);
        assert_eq!(r.record["broken_taxa"], json!(["Pongo_abelii"]));
        assert_eq!(r.record["synth_id"], "opentree16.1");
        assert!(r.record["induced_subtree_sha256"].as_str().unwrap().len() == 64);
    }

    #[test]
    fn strains_of_one_species_make_the_match_incomplete() {
        let m = ReferenceMatch {
            names: NameMatches {
                taxonomy_version: "3.7".into(),
                ott: BTreeMap::from([
                    ("E_coli_K12".to_string(), 474506),
                    ("E_coli_O157".to_string(), 474506),
                ]),
                matched_names: BTreeMap::new(),
                unmatched: vec!["Strain_X".into()],
                ambiguous: vec![],
                query: Value::Null,
                reply_sha256: String::new(),
            },
            same_taxon: vec![(474506, vec!["E_coli_K12".into(), "E_coli_O157".into()])],
        };
        assert!(!m.is_complete(3));
        let report = m.report().join("\n");
        assert!(report.contains("not found: Strain_X"));
        assert!(report.contains("the same taxon (ott474506) for: E_coli_K12, E_coli_O157"));
        assert_eq!(
            species_name("Escherichia coli K-12 MG1655"),
            "Escherichia coli"
        );
    }

    #[test]
    fn a_taxon_that_contains_another_is_refused() {
        let tree = qmaws_core::newick::Tree::parse("((ott1,ott2)ott3,ott4,ott5);").unwrap();
        let labels: BTreeMap<String, String> = [
            ("ott1", "a"),
            ("ott2", "b"),
            ("ott3", "genus"),
            ("ott4", "c"),
            ("ott5", "d"),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        assert!(taxa_newick(&tree, &labels)
            .unwrap_err()
            .contains("genus is an inner node"));
    }

    #[test]
    fn search_names_read_underscores_as_spaces() {
        assert_eq!(
            search_name("Oreochromis_sp-KM2006"),
            "Oreochromis sp-KM2006"
        );
    }
}
