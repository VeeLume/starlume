//! Store pictures for game hulls. RSI's pledge store and the game data share
//! no id, so a hull finds its store entry by name: the game spells it with
//! its maker ("RSI Constellation Mk5 Centaurus"), the store without and in
//! roman numerals ("Constellation Mk V Centaurus"). Both are reduced to
//! plain lowercase words with `Mk` numbers as digits; a hull matches the
//! entry whose words are its own after zero to three leading maker words.
//! An edition the store does not list takes its base hull's picture.

use std::collections::HashMap;

use crate::ShipRow;

/// Store entries whose name the rule above cannot reach: (game name as
/// [`words`] reduces it, store name the same way). LIVE 4.10 against the
/// store of 2026-10-21; the F8A Lightning, the CitizenCon Mustang and the
/// Command Module have no store entry.
const OVERRIDES: &[(&str, &str)] = &[
    ("anvil c8r pisces rescue", "c8r pisces"),
    (
        "anvil f7c m hornet heartseeker mk 1",
        "f7c m super hornet heartseeker mk 1",
    ),
    ("crusader a2 hercules starlifter", "a2 hercules"),
    ("crusader c2 hercules starlifter", "c2 hercules"),
    ("crusader m2 hercules starlifter", "m2 hercules"),
    ("crusader ares star fighter inferno", "ares inferno"),
    ("crusader ares star fighter ion", "ares ion"),
    ("crusader mercury star runner", "mercury"),
    ("drake cutlass 2949 best in show edition", "cutlass black"),
    ("drake dragonfly", "dragonfly black"),
    ("origin 600i", "600i explorer"),
    ("origin 85x limited", "85x"),
    ("origin m50 interceptor", "m50"),
];

/// `name` as lowercase words, `Mk V` / `Mk. V` / `MkV` as `mk 5`.
pub fn words(name: &str) -> Vec<String> {
    let raw: Vec<String> = name
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect();
    let roman = |w: &str| match w {
        "i" => Some("1"),
        "ii" => Some("2"),
        "iii" => Some("3"),
        "iv" => Some("4"),
        "v" => Some("5"),
        "vi" => Some("6"),
        _ => None,
    };
    let mut out = Vec::new();
    let mut i = 0;
    while i < raw.len() {
        let w = &raw[i];
        if w == "mk" {
            out.push("mk".to_string());
            if let Some(n) = raw.get(i + 1) {
                out.push(roman(n).map_or_else(|| n.clone(), str::to_string));
                i += 2;
                continue;
            }
        } else if let Some(rest) = w.strip_prefix("mk")
            && !rest.is_empty()
            && (rest.chars().all(|c| c.is_ascii_digit()) || roman(rest).is_some())
        {
            out.push("mk".to_string());
            out.push(roman(rest).map_or_else(|| rest.to_string(), str::to_string));
        } else {
            out.push(w.clone());
        }
        i += 1;
    }
    out
}

/// Each hull's store entry: game GUID → store id. `store` is (id, name).
pub fn match_store(ships: &[ShipRow], store: &[(u32, &str)]) -> HashMap<String, u32> {
    let by_words: HashMap<String, u32> = store
        .iter()
        .map(|(id, name)| (words(name).join(" "), *id))
        .collect();
    let overrides: HashMap<&str, &str> = OVERRIDES.iter().copied().collect();
    let mut out = HashMap::new();
    for s in ships {
        let w = words(&s.name);
        let joined = w.join(" ");
        let hit = overrides
            .get(joined.as_str())
            .and_then(|o| by_words.get(*o))
            .or_else(|| {
                (0..=3.min(w.len().saturating_sub(1))).find_map(|k| by_words.get(&w[k..].join(" ")))
            });
        if let Some(id) = hit {
            out.insert(s.guid.clone(), *id);
        }
    }
    // Editions the store does not list: the base hull's picture.
    for s in ships {
        if !out.contains_key(&s.guid)
            && let Some(id) = s.base.as_ref().and_then(|b| out.get(b)).copied()
        {
            out.insert(s.guid.clone(), id);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mk_numbers_read_alike() {
        assert_eq!(
            words("Constellation Mk V Centaurus"),
            words("Constellation Mk5 Centaurus")
        );
        assert_eq!(words("Mk. II"), vec!["mk", "2"]);
        assert_eq!(words("MkII"), vec!["mk", "2"]);
        assert_eq!(
            words("F7C-M Super Hornet Mk II"),
            vec!["f7c", "m", "super", "hornet", "mk", "2"]
        );
    }
}
