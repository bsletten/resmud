//! `urn:resmud:query`: a stored SPARQL query with one NAME bound into it, answered as text.
//!
//! **Why this is code.** Every question the game asks of the world (where am I, what is here,
//! where does the east exit go) is a stored query with one name in it, and a named request is
//! how a name with a variable in it is bound to one (`urn:resmud:room:id:{room}:exit`). But a
//! named request fixes an argument as a value, a reference, a reference template or the WHOLE
//! text a variable captured, and `urn:sparql:select` takes a query's variables only as a JSON
//! object (`bindings=`). Nothing in between builds that object, and splicing the name into the
//! query text would be the injection `bindings=` exists to prevent. So this endpoint does the
//! one thing that is missing: it takes `subject` as a NAME (the kernel hands it the IRI's text,
//! since the input is `xsd:anyURI`), binds it as `?subject`, runs the query, and answers the
//! `?line` column, one value per line (or joined by `separator`, a space for a line of
//! sentences), in the query's own order, with no trailing newline: the shape a text template
//! splices with `$r{…}`. It reads nothing itself; the query, the graphs and the answer all go
//! through the kernel, so it is exactly as cacheable as what it read.

use ikigai_core::{
    ArgRef, ArgSpec, AsyncFnEndpoint, Description, Error, InputGroup, Invocation, Iri, ReprType,
    Representation, Request, Result, Verb,
};
use serde_json::{json, Value};

const XSD_STRING: &str = "http://www.w3.org/2001/XMLSchema#string";
const XSD_ANY_URI: &str = "http://www.w3.org/2001/XMLSchema#anyURI";

/// A SPARQL results term naming an IRI, the shape `bindings=` takes.
pub fn uri(iri: &str) -> Value {
    json!({ "type": "uri", "value": iri })
}

/// The binder endpoint, bound at `urn:resmud:query`.
pub fn binder() -> AsyncFnEndpoint {
    AsyncFnEndpoint::new("resmud-query", |inv| {
        Box::pin(async move {
            let query = inv.inline_str("query")?.to_string();
            let graph =
                match (
                    inv.request.args.contains_key("graph"),
                    inv.request.args.contains_key("atom"),
                ) {
                    (true, false) => inv.inline_str("graph")?.to_string(),
                    (false, true) => name_arg(inv, "atom")?.as_str().to_string(),
                    _ => return Err(Error::InvalidArgument {
                        name: "graph".into(),
                        detail:
                            "give exactly one of `graph` (a list of names) and `atom` (one name)"
                                .into(),
                    }),
                };
            let subject = name_arg(inv, "subject")?;
            let lines = select(
                inv,
                ArgRef::Inline(query.into_bytes()),
                &graph,
                json!({ "subject": uri(subject.as_str()) }),
            )
            .await?;
            // The rows are joined in the order the query answered them, which is its ORDER BY's.
            let separator = if inv.request.args.contains_key("separator") {
                inv.inline_str("separator")?.to_string()
            } else {
                "\n".to_string()
            };
            Ok(text(lines.join(&separator)).cacheable())
        })
    })
    .with_description(
        Description::new("resmud-query")
            .title("A stored query, with one name bound")
            .summary(
                "Runs a stored SPARQL SELECT over `graph` with `subject` bound to `?subject` as \
                 an IRI (never spliced into the text), and answers its `?line` column as plain \
                 text, one value per line (or joined by `separator`), in the query's order.",
            )
            .verb(Verb::Source)
            .verb(Verb::Meta)
            .input(
                ArgSpec::new("query")
                    .summary(
                        "the SPARQL SELECT; a reference to a stored query is read for its text",
                    )
                    .class(XSD_STRING),
            )
            .input(
                ArgSpec::new("graph")
                    .summary(
                        "the graphs to query, as names separated by whitespace; a reference to \
                         a list of names (urn:resmud:world) is read for the list",
                    )
                    .class(XSD_STRING)
                    .optional(),
            )
            .input(
                ArgSpec::new("atom")
                    .summary("instead of `graph`: the one atom to query, by name")
                    .class(XSD_ANY_URI)
                    .optional(),
            )
            .group(InputGroup::exactly_one(["graph", "atom"]))
            .input(
                ArgSpec::new("subject")
                    .summary("the name bound to ?subject")
                    .class(XSD_ANY_URI),
            )
            .input(
                ArgSpec::new("separator")
                    .summary(
                        "what joins the ?line values, in the order the query answers them; a \
                         newline when it is left out, a space to make one line of sentences",
                    )
                    .class(XSD_STRING)
                    .optional(),
            )
            .output("text/plain;charset=utf-8"),
    )
}

/// `name`, an argument that must be an IRI, or `InvalidArgument` naming it.
pub(crate) fn name_arg(inv: &Invocation<'_>, name: &str) -> Result<Iri> {
    let raw = inv.inline_str(name)?;
    Iri::parse(raw.trim()).map_err(|e| Error::InvalidArgument {
        name: name.to_string(),
        detail: format!("`{raw}` is not a name: {e}"),
    })
}

/// A `text/plain; charset=utf-8` answer.
pub fn text(s: impl Into<String>) -> Representation {
    Representation::new(
        ReprType::new("text/plain").with_param("charset", "utf-8"),
        s.into().into_bytes(),
    )
}

/// Run a SELECT (`query`: inline text or a reference to a stored query) over `graph` with
/// `bindings`, and answer the values of its `?line` column, in order.
pub async fn select(
    inv: &Invocation<'_>,
    query: ArgRef,
    graph: &str,
    bindings: Value,
) -> Result<Vec<String>> {
    let request = Request::new(Verb::Source, crate::iri("urn:sparql:select"))
        .with_arg("query", query)
        .with_arg("graph", ArgRef::Inline(graph.as_bytes().to_vec()))
        .with_arg(
            "bindings",
            ArgRef::Inline(bindings.to_string().into_bytes()),
        );
    let answer = inv.issue(request).await?;
    lines(&answer.bytes)
}

/// Run a stored CONSTRUCT over `graph` with `bindings`, and answer its Turtle.
pub async fn construct(
    inv: &Invocation<'_>,
    query: &str,
    graph: &str,
    bindings: Value,
) -> Result<String> {
    let request = Request::new(Verb::Source, crate::iri("urn:sparql:construct"))
        .with_arg(
            "query",
            ArgRef::Reference(crate::iri(&format!("urn:resmud:query:id:{query}"))),
        )
        .with_arg("graph", ArgRef::Inline(graph.as_bytes().to_vec()))
        .with_arg(
            "bindings",
            ArgRef::Inline(bindings.to_string().into_bytes()),
        )
        .with_arg("as", ArgRef::Inline(b"text/turtle".to_vec()));
    let answer = inv.issue(request).await?;
    String::from_utf8(answer.bytes)
        .map_err(|_| Error::Endpoint("a CONSTRUCT answered something other than UTF-8".into()))
}

/// The `?line` values of a SPARQL results JSON document, in order; a solution that leaves
/// `?line` unbound contributes nothing.
fn lines(bytes: &[u8]) -> Result<Vec<String>> {
    let doc: Value = serde_json::from_slice(bytes).map_err(|e| {
        Error::Endpoint(format!("a SELECT answered something that is not JSON: {e}"))
    })?;
    let rows = doc
        .pointer("/results/bindings")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::Endpoint("a SELECT answered JSON without results.bindings".into()))?;
    Ok(rows
        .iter()
        .filter_map(|row| row.pointer("/line/value").and_then(Value::as_str))
        .map(str::to_string)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_reads_the_line_column_in_order_and_skips_unbound_rows() {
        let doc = br#"{"head":{"vars":["line"]},"results":{"bindings":[
            {"line":{"type":"literal","value":"first"}},
            {},
            {"line":{"type":"literal","value":"second"}}]}}"#;
        assert_eq!(lines(doc).unwrap(), ["first", "second"]);
    }

    #[test]
    fn lines_refuses_a_document_that_is_not_results_json() {
        assert!(matches!(lines(b"not json"), Err(Error::Endpoint(_))));
        assert!(matches!(lines(br#"{"head":{}}"#), Err(Error::Endpoint(_))));
    }
}
