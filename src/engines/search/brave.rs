use scraper::{ElementRef, Selector};
use url::Url;

use crate::{
    engines::{EngineResponse, RequestResponse, CLIENT},
    parse::{parse_html_response_with_opts, ParseOpts, QueryMethod},
};

const BRAVE_BASE_URL: &str = "https://search.brave.com";

pub async fn request(query: &str) -> RequestResponse {
    CLIENT
        .get(Url::parse_with_params("https://search.brave.com/search", &[("q", query)]).unwrap())
        .into()
}

pub fn parse_response(body: &str) -> eyre::Result<EngineResponse> {
    parse_html_response_with_opts(
        body,
        ParseOpts::new()
            .result("main .snippet[data-pos]:not(.standalone)")
            .title(".title")
            .href(QueryMethod::Manual(Box::new(|el: &ElementRef| {
                let url = el
                    .select(&Selector::parse("a[href]").unwrap())
                    .next()
                    .and_then(|n| n.value().attr("href"))
                    .unwrap_or_default();
                if url.starts_with('/') {
                    Ok(format!("{BRAVE_BASE_URL}{url}"))
                } else {
                    Ok(url.to_string())
                }
            })))
            .description(".generic-snippet, .video-snippet > .snippet-description"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_response_prefixes_relative_go_url() {
        let html = r#"
            <main>
                <div class="snippet" data-pos="1">
                    <a href="/go/v1/AflM11a7SDWyav2x">
                        <div class="title">Noctalia</div>
                    </a>
                    <div class="generic-snippet">A family of native Wayland projects.</div>
                </div>
            </main>
        "#;

        let res = parse_response(html).unwrap();
        assert_eq!(res.search_results.len(), 1);
        assert_eq!(
            res.search_results[0].url,
            "https://search.brave.com/go/v1/AflM11a7SDWyav2x"
        );
        assert_eq!(res.search_results[0].title, "Noctalia");
    }
}
