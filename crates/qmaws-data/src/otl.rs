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
    let names: Vec<&str> = taxa.iter().map(|(_, s)| s.as_str()).collect();
    let query = json!({"names": names, "do_approximate_matching": false}).to_string();
    let url = format!("{API}/tnrs/match_names");
    let text = poster.post_json(&url, &query)?;
    let reply: Value =
        serde_json::from_str(&text).map_err(|e| format!("Open Tree of Life answer: {e}"))?;
    let version = reply["taxonomy"]["version"]
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
    let mut ott: BTreeMap<String, u64> = BTreeMap::new();
    let (mut unmatched, mut ambiguous) = (Vec::new(), Vec::new());
    for r in results {
        let name = r["name"].as_str().unwrap_or("");
        let Some(&run) = by_search.get(name) else {
            continue;
        };
        let matches = r["matches"].as_array().map(Vec::as_slice).unwrap_or(&[]);
        match matches {
            [] => unmatched.push(run.to_string()),
            [one] => match one["taxon"]["ott_id"].as_u64() {
                Some(id) => {
                    ott.insert(run.to_string(), id);
                }
                None => unmatched.push(run.to_string()),
            },
            _ => ambiguous.push(run.to_string()),
        }
    }
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
        "match_names_query": serde_json::from_str::<Value>(&query).unwrap_or(Value::Null),
        "match_names_sha256": sha256(&text_of(&reply)),
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

    #[test]
    fn search_names_read_underscores_as_spaces() {
        assert_eq!(
            search_name("Oreochromis_sp-KM2006"),
            "Oreochromis sp-KM2006"
        );
    }
}
