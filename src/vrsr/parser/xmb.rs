use super::super::error::Error;
use super::super::{EpisodeInfo, ResourceInfo, TeleplayInfo, TeleplaySrc, URIType, Uri};
use super::super::{EpisodeParse, GenerateInfo, Request, ResourceParse, TeleplayParse};
use scraper::{Html, Selector};
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct XMBParser {
    info: ResourceInfo,
}

impl Default for XMBParser {
    fn default() -> Self {
        Self {
            info: ResourceInfo {
                name: "小目标".to_string(),
                host: "https://tv.xmb.app/index.php".to_string(),
                search_path: "index.php/vod/search.html".to_string(),
                search_key: "wd".to_string(),
            },
        }
    }
}

impl XMBParser {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl GenerateInfo for XMBParser {
    fn generate_resource_info(&self) -> ResourceInfo {
        self.info.clone()
    }

    fn generate_teleplay_info(&self, id: u64) -> TeleplayInfo {
        let mut host_url = url::Url::parse(&self.info.host).unwrap();
        host_url.set_path(&format!("/index.php/vod/detail/id/{}.html", id));
        TeleplayInfo {
            id,
            home_page: host_url.to_string(),
            ..TeleplayInfo::default()
        }
    }
}

impl ResourceParse for XMBParser {
    async fn parse(
        &self,
        html: &str,
        _org_rul: &str,
        _requestor: Arc<impl Request>,
    ) -> Result<Vec<TeleplayInfo>, Error> {
        let html = Html::parse_document(&html);
        let mut infos: Vec<TeleplayInfo> = Vec::new();

        let search_list_selector = Selector::parse("div.module-items.module-card-items div.module-card-item.module-item")?;
        let title_selector =
            Selector::parse("div.module-card-item-info div.module-card-item-title a strong")?;
        let home_page_selector =
            Selector::parse("a")?;
        let cover_selector = 
            Selector::parse("a div.module-item-pic img")?;
        let status_selector = 
            Selector::parse("a div.module-item-note")?;
        let other_selector = 
            Selector::parse("div.module-card-item-info div.module-info-item div.module-info-item-content")?;

        let teleplays = html.select(&search_list_selector);
        for teleplay in teleplays {
            let mut info = TeleplayInfo::default();
            info.title = teleplay
                .select(&title_selector)
                .next()
                .ok_or_else(|| Error::ParseError("Failed to find name".to_string()))?
                .inner_html();
            info.home_page = teleplay
                .select(&home_page_selector)
                .next()
                .ok_or_else(|| Error::ParseError("Failed to find home page".to_string()))?
                .value()
                .attr("href")
                .ok_or_else(|| Error::ParseError("Failed to find home page".to_string()))?
                .to_string();
            info.id = info
                .home_page
                .split('/')
                .last()
                .unwrap()
                .split('.')
                .take(1)
                .map(|v| v.parse::<u64>().unwrap())
                .last()
                .unwrap();

            if let Some(cover) = teleplay.select(&cover_selector).next() {
                if let Some(cover) = cover.value().attr("data-original") {
                    info.cover.replace(cover.to_string());
                }
            }
            if let Some(status) = teleplay.select(&status_selector).next() {
                info.status.replace(status.inner_html());
            }

            let mut others = teleplay.select(&other_selector);
            if let Some(content0) = others.next() {
                let mut values = content0.children()
                    .map(|v| v.value().as_text())
                    .filter(|v| v.is_some())
                    .map(|v| v.unwrap().trim().to_string())
                    .collect::<Vec<_>>();

                if let Some(genre) = values.pop() {
                    info.genre.replace(genre);
                }
                if let Some(region) = values.pop() {
                    info.region.replace(region);
                }
                if let Some(times) = values.pop() {
                    info.times.replace(times);
                }
            }
            if let Some(content1) = others.next() {
                let starring = content1.inner_html().split(',').map(|v| v.trim().to_string()).collect::<Vec<_>>();
                if !starring.is_empty() {
                    info.starring.replace(starring);
                }
            }
            infos.push(info);
        }
        Ok(infos)
    }
}

impl TeleplayParse for XMBParser {
    async fn parse(
        &self,
        html: &str,
        _org_rul: &str,
        teleplay_info: &mut TeleplayInfo,
        _requestor: Arc<impl Request>,
    ) -> Result<Vec<TeleplaySrc>, Error> {
        let html = Html::parse_document(&html);
        let info_selector = Selector::parse("div.module-info-main")?;
        if let Some(info) = html.select(&info_selector).next() {
            let title_selector = Selector::parse("div.module-info-heading h1")?;
            if let Some(title) = info.select(&title_selector).next() {
                if teleplay_info.title.is_empty() {
                    teleplay_info.title = title.inner_html().trim().to_string();
                }
            }
            let others_selector = Selector::parse("div.module-info-heading div.module-info-tag div.module-info-tag-link a")?;
            let mut values = info.select(&others_selector).map(|v| v.inner_html().trim().to_string()).collect::<Vec<_>>();
            if let Some(genre) = values.pop() {
                teleplay_info.genre.replace(genre);
            }
            if let Some(region) = values.pop() {
                teleplay_info.region.replace(region);
            }
            if let Some(times) = values.pop() {
                teleplay_info.times.replace(times);
            }

            let introduction_selector =
                Selector::parse("div.module-info-content div.module-info-item.module-info-introduction div.module-info-introduction-content p")?;
            let director_selector =
                Selector::parse("div.module-info-content div.module-info-items div:nth-child(4) a")?;
            let starring_selector =
                Selector::parse("div.module-info-content div.module-info-items div:nth-child(5) a")?;
            let update_time_selector =
                Selector::parse("div.module-info-content div.module-info-items div.module-info-item p.module-info-item-content")?;

            if let Some(introduction) = info.select(&introduction_selector).next() {
                teleplay_info.introduction.replace(introduction.inner_html());
            }
            if let Some(update_time) = info.select(&update_time_selector).next() {
                teleplay_info.update_time.replace(update_time.inner_html());
            }
            let director = html.select(&director_selector).map(|v| v.inner_html().trim().to_string()).collect::<Vec<_>>();
            let starring = html.select(&starring_selector).map(|v| v.inner_html().trim().to_string()).collect::<Vec<_>>();
            if !director.is_empty() {
                teleplay_info.director.replace(director);
            }
            if !starring.is_empty() {
                teleplay_info.starring.replace(starring);
            }
        }

        let mut sources: Vec<TeleplaySrc> = Vec::new();
        let srcs_name_selector = Selector::parse("div.module div.module-tab div.module-tab-items div.module-tab-items-box div.module-tab-item.tab-item span")?;
        let srcs_selector = Selector::parse("div.module-play-list")?;
        let uri_selector = Selector::parse("a")?;
        let srcs = html.select(&srcs_selector);
        let name_selector = Selector::parse("span")?;
        let srcs_name = html.select(&srcs_name_selector);
        for (src, name) in srcs.zip(srcs_name) {
            let mut source: TeleplaySrc = TeleplaySrc::new();
            source.set_name(name.inner_html().trim());
            let urls = src.select(&uri_selector);
            for url in urls {
                let info = EpisodeInfo {
                    name: url.select(&name_selector).next()
                        .ok_or_else(|| Error::ParseError("Failed to find episode name".to_string()))?
                        .inner_html().trim().to_string(),
                    url: url
                        .value()
                        .attr("href")
                        .ok_or_else(|| Error::ParseError("Failed to find episode url".to_string()))?
                        .to_string(),
                };
                source.append_episode(info);
            }
            sources.push(source);
        }
        Ok(sources)
    }
}

impl EpisodeParse for XMBParser {
    async fn parse(&self, html: &str, _org_rul: &str, _requestor: Arc<impl Request>) -> Result<Uri, Error> {
        let html = Html::parse_document(html);
        let m3u8_selector = Selector::parse("div.module-main div.player-box div.player-box-main script")?;
        let m3u8_json = html
            .select(&m3u8_selector)
            .next()
            .ok_or_else(|| Error::ParseError("Failed to find m3u8 json msg".to_string()))?
            .inner_html()
            .split("=")
            .skip(1)
            .collect::<Vec<_>>()
            .join("")
            .trim()
            .to_string();
        let msg: Value = serde_json::from_str(&m3u8_json)?;
        Ok(Uri {
            uri: msg["url"]
                .as_str()
                .ok_or_else(|| {
                    Error::ParseError("Faild to find url string from json msg".to_string())
                })?
                .trim_matches('"')
                .to_string(),
            utype: URIType::M3U8,
        })
    }
}
