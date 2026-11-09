//! The game's own code-as-data: the stored SPARQL queries (`assets/query/*.rq`) and the text
//! templates (`assets/template/*.txt`), each served at a name so a composition can name it and
//! a golden thread can hang from it. They ship with the crate, so they are cacheable for good.

use ikigai_core::{ArgSpec, Description, Error, FnEndpoint, ReprType, Representation, Verb};

const XSD_STRING: &str = "http://www.w3.org/2001/XMLSchema#string";

/// Every stored query, by the key it is named under: `urn:resmud:query:id:{query}`.
pub const QUERIES: &[(&str, &str)] = &[
    ("affordance", include_str!("../assets/query/affordance.rq")),
    (
        "carries-graph",
        include_str!("../assets/query/carries-graph.rq"),
    ),
    ("content", include_str!("../assets/query/content.rq")),
    (
        "content-graph",
        include_str!("../assets/query/content-graph.rq"),
    ),
    (
        "description",
        include_str!("../assets/query/description.rq"),
    ),
    ("exit", include_str!("../assets/query/exit.rq")),
    ("exit-to", include_str!("../assets/query/exit-to.rq")),
    ("grant", include_str!("../assets/query/grant.rq")),
    ("holds", include_str!("../assets/query/holds.rq")),
    ("inventory", include_str!("../assets/query/inventory.rq")),
    ("patch", include_str!("../assets/query/patch.rq")),
    (
        "patch-holding",
        include_str!("../assets/query/patch-holding.rq"),
    ),
    ("place", include_str!("../assets/query/place.rq")),
    ("put", include_str!("../assets/query/put.rq")),
    ("relocate", include_str!("../assets/query/relocate.rq")),
    ("remove", include_str!("../assets/query/remove.rq")),
    ("sees", include_str!("../assets/query/sees.rq")),
    ("subject", include_str!("../assets/query/subject.rq")),
    ("validate", include_str!("../assets/query/validate.rq")),
];

/// Every text template, by the key it is named under: `urn:resmud:template:id:{template}`.
pub const TEMPLATES: &[(&str, &str)] = &[
    ("look", include_str!("../assets/template/look.txt")),
    ("room", include_str!("../assets/template/room.txt")),
    (
        "sees-graph",
        include_str!("../assets/template/sees-graph.txt"),
    ),
];

fn served(
    name: &'static str,
    key: &'static str,
    table: &'static [(&'static str, &'static str)],
    media: &'static str,
    summary: &'static str,
) -> FnEndpoint {
    FnEndpoint::new(name, move |inv| {
        let wanted = inv.bindings.get(key).unwrap_or_default();
        let (_, body) = table
            .iter()
            .find(|(k, _)| *k == wanted)
            .ok_or_else(|| Error::NotFound(format!("there is no stored {key} called {wanted}")))?;
        Ok(Representation::new(
            ReprType::new(media).with_param("charset", "utf-8"),
            body.as_bytes().to_vec(),
        )
        .cacheable())
    })
    .with_description(
        Description::new(name)
            .title(summary)
            .summary(summary)
            .verb(Verb::Source)
            .verb(Verb::Meta)
            .input(
                ArgSpec::new(key)
                    .summary(format!("the {key}'s key"))
                    .class(XSD_STRING)
                    .one_of(table.iter().map(|(k, _)| *k))
                    .binding(),
            )
            .output(format!("{media};charset=utf-8")),
    )
}

/// `urn:resmud:query:id:{query}`: a stored SPARQL query's text.
pub fn queries() -> FnEndpoint {
    served(
        "resmud-query-text",
        "query",
        QUERIES,
        "application/sparql-query",
        "A stored ResMUD query",
    )
}

/// `urn:resmud:template:id:{template}`: a text template, filled by `urn:iki:fn:compose`.
pub fn templates() -> FnEndpoint {
    served(
        "resmud-template",
        "template",
        TEMPLATES,
        "text/plain",
        "A ResMUD text template",
    )
}
