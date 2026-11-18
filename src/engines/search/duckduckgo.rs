use scraper::{Html, Selector};
use url::Url;

use crate::{
    engines::{EngineResponse, EngineSearchResult, RequestResponse, CLIENT},
    urls::normalize_url,
};

pub async fn request(query: &str) -> RequestResponse {
    CLIENT
        .get(Url::parse_with_params("https://lite.duckduckgo.com/lite/", &[("q", query)]).unwrap())
        .into()
}

fn extract_url(href: &str) -> String {
    let full = if href.starts_with("//") {
        format!("https:{href}")
    } else {
        href.to_string()
    };

    if let Ok(parsed) = Url::parse(&full) {
        if let Some((_, uddg)) = parsed.query_pairs().find(|(k, _)| k == "uddg") {
            return normalize_url(&uddg);
        }
    }

    normalize_url(&full)
}

pub fn parse_response(body: &str) -> eyre::Result<EngineResponse> {
    let dom = Html::parse_document(body);
    let tr_sel = Selector::parse("tr").unwrap();
    let link_sel = Selector::parse("a.result-link[href]").unwrap();
    let snippet_sel = Selector::parse("td.result-snippet").unwrap();

    let mut search_results = Vec::new();
    let mut pending: Option<(String, String)> = None;

    for tr in dom.select(&tr_sel) {
        let is_ad = tr
            .value()
            .classes()
            .any(|c| c == "result-sponsored" || c == "result--ad");
        if is_ad {
            pending = None;
            continue;
        }

        if let Some(link) = tr.select(&link_sel).next() {
            let href = link.value().attr("href").unwrap_or_default();
            if href.contains("duckduckgo.com/y.js") {
                pending = None;
                continue;
            }
            let title = link.text().collect::<String>().trim().to_string();
            let url = extract_url(href);
            pending = Some((url, title));
        } else if let Some(snippet) = tr.select(&snippet_sel).next() {
            if let Some((url, title)) = pending.take() {
                let description = snippet.text().collect::<String>().trim().to_string();
                if !title.is_empty() && !description.is_empty() {
                    search_results.push(EngineSearchResult {
                        url,
                        title,
                        description,
                    });
                }
            }
        }
    }

    Ok(EngineResponse {
        search_results,
        featured_snippet: None,
        answer_html: None,
        infobox_html: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_response_extracts_uddg_results() {
        let html = r#"
            <table>
                <tr>
                    <td>1.</td>
                    <td>
                        <a rel="nofollow" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fnoctalia.dev%2F&amp;rut=abc" class="result-link">
                            Noctalia - A Family of Native Wayland Projects
                        </a>
                    </td>
                </tr>
                <tr>
                    <td></td>
                    <td class="result-snippet">
                        <b>Noctalia</b> - a family of native Wayland projects.
                    </td>
                </tr>
                <tr class="result-sponsored">
                    <td></td>
                    <td>
                        <a href="//duckduckgo.com/y.js?ad=1" class="result-link">Ad Result</a>
                    </td>
                </tr>
                <tr>
                    <td></td>
                    <td class="result-snippet">Ad description</td>
                </tr>
            </table>
        "#;

        let res = parse_response(html).unwrap();
        assert_eq!(res.search_results.len(), 1);
        assert_eq!(res.search_results[0].url, "https://noctalia.dev");
        assert_eq!(
            res.search_results[0].title,
            "Noctalia - A Family of Native Wayland Projects"
        );
        assert_eq!(
            res.search_results[0].description,
            "Noctalia - a family of native Wayland projects."
        );
    }
}
