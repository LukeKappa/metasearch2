use futures::future::join_all;
use scraper::{ElementRef, Html, Selector};
use tracing::warn;
use url::Url;

use crate::{
    engines::{EngineResponse, RequestResponse, CLIENT},
    parse::{parse_html_response_with_opts, ParseOpts, QueryMethod},
    urls::normalize_url,
};

const BRAVE_BASE_URL: &str = "https://search.brave.com";
const BRAVE_GO_PREFIX: &str = "https://search.brave.com/go/v1/";

pub async fn request(query: &str) -> eyre::Result<RequestResponse> {
    let url = Url::parse_with_params("https://search.brave.com/search", &[("q", query)]).unwrap();
    let body = CLIENT.get(url).send().await?.text().await?;
    let mut response = parse_response(&body)?;

    let resolve_futures = response.search_results.iter().map(|result| async {
        if result.url.starts_with(BRAVE_GO_PREFIX) {
            resolve_go_url(&result.url).await
        } else {
            None
        }
    });

    let resolved_urls = join_all(resolve_futures).await;
    for (result, resolved) in response.search_results.iter_mut().zip(resolved_urls) {
        if let Some(url) = resolved {
            result.url = url;
        }
    }

    Ok(RequestResponse::Instant(Box::new(response)))
}

async fn resolve_go_url(go_url: &str) -> Option<String> {
    let res = match CLIENT.get(go_url).send().await {
        Ok(res) => res,
        Err(e) => {
            warn!("failed to resolve brave cloaked url {go_url}: {e}");
            return None;
        }
    };

    let body = match res.text().await {
        Ok(body) => body,
        Err(e) => {
            warn!("failed to read brave cloaked url response {go_url}: {e}");
            return None;
        }
    };

    let resolved = parse_go_response(&body);
    if resolved.is_none() {
        warn!("failed to extract destination url from brave cloaked page {go_url}");
    }
    resolved
}

fn parse_go_response(body: &str) -> Option<String> {
    let dom = Html::parse_document(body);
    let selector = Selector::parse("a.destination[href]").unwrap();
    let href = dom.select(&selector).next()?.value().attr("href")?;
    Some(normalize_url(href))
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

    #[test]
    fn test_parse_go_response_extracts_destination() {
        let html = r#"
            <html><body>
                <p>The link you followed is sending you to
                    <strong class="host">noctalia.dev</strong>:
                    <a class="destination svelte-12edz7s" href="https://noctalia.dev/">https://noctalia.dev/</a>
                </p>
            </body></html>
        "#;

        assert_eq!(
            parse_go_response(html).as_deref(),
            Some("https://noctalia.dev")
        );
    }
}
