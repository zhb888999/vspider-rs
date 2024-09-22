use super::super::{ResourceInfo, TeleplayInfo, EpisodeInfo, Uri, URIType};
use super::super::{GenerateResourceInfo, ResourceParse, TeleplayParse, EpisodeParse, Request};
use scraper::{Html, Selector};
use std::collections::HashMap;
use super::super::error::Error;
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct IJUJITVParser {
    info: ResourceInfo,
}

impl IJUJITVParser {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            info: ResourceInfo {
                name: "剧集TV".to_string(),
                host: "https://v.ijujitv.cc".to_string(),
                search_path: "search/-------------.html".to_string(),
                search_key: "wd".to_string(),
            },
        })
    }

    #[allow(unused)]
    fn parse_info(&self, html: &Html) -> HashMap<String, String> {
        let info_selector = Selector::parse("div.tv-bd p:has(b)").unwrap();
        let infos = html.select(&info_selector);
        let mut info_dict: HashMap<String, String> = HashMap::new();
        for info in infos {
            let texts = info.text().collect::<Vec<_>>();
            if texts.len() < 2 { continue; }
            let key = texts[0].trim().split("：").collect::<Vec<_>>()[0].to_string();
            let value = texts[1].trim().to_string();
            info_dict.insert(key, value);
        }
        info_dict
    }
}

impl GenerateResourceInfo for IJUJITVParser {
    fn generate(&self) -> ResourceInfo {
        self.info.clone()
    }
}

impl ResourceParse for IJUJITVParser {
    async fn parse(&self, html: &str, _requestor: Arc<impl Request>) -> Result<Vec<TeleplayInfo>, Error> {
        // println!("{:?}", html);
        // panic!("not implemented");
        let html = Html::parse_document(&html);

        let mut result: Vec<TeleplayInfo> = Vec::new();

        let search_selector = Selector::parse("div.m-list-inner")?;
        let name_selector = Selector::parse("ul.m-list li.m-item a.thumb")?;
        let url_selector = Selector::parse("ul.m-list li.m-item a.thumb")?;
        let films = html.select(&search_selector);
        for film in films {
            let fname = film.select(&name_selector)
                .next()
                .ok_or_else(|| Error::ParseError("Failed to find title".to_string()))?
                .value().attr("title")
                .ok_or_else(|| Error::ParseError("Failed to find title".to_string()))?;
            let furl = film.select(&url_selector)
                .next()
                .ok_or_else(|| Error::ParseError("Failed to find home page".to_string()))?
                .value().attr("href")
                .ok_or_else(|| Error::ParseError("Failed to find home page".to_string()))?;
            let mut info = TeleplayInfo::default();
            info.home_page = furl.to_string();
            info.title = fname.to_string();
            result.push(info);
        }

        Ok(result)
    }
}

impl TeleplayParse for IJUJITVParser {
    async fn parse(&self, html: &str, _teleplay_info: &mut TeleplayInfo, _requestor: Arc<impl Request>) -> Result<Vec<Vec<EpisodeInfo>>, Error> {
        println!("{:?}", html);
        let html = Html::parse_document(&html);
        let mut sources: Vec<Vec<EpisodeInfo>> = Vec::new();
        let srcs_selector = Selector::parse("div.tab-content.stui-pannel_bd.col-pd.clearfix ul")?;
        let uri_selector = Selector::parse("li a")?;
        let srcs = html.select(&srcs_selector);
        for src in srcs {
            let mut source: Vec<EpisodeInfo> = Vec::new();
            let urls = src.select(&uri_selector);
            for url in urls {
                let href = url.value()
                    .attr("href")
                    .ok_or_else(|| Error::ParseError("Failed to find episode url".to_string()))?;
                if href.starts_with("//") {
                    continue;
                }
                let mut info = EpisodeInfo::default();
                info.url = href.to_string();
                source.push(info); 
            }
            sources.push(source);
            break;
        }
        Ok(sources)
    }
}

impl EpisodeParse for IJUJITVParser {
    async fn parse(&self, html: &str, _requestor: Arc<impl Request>) -> Result<Uri, Error> {
        let html = Html::parse_document(html);
        let m3u8_selector = Selector::parse("div.playBox script")?;
        let m3u8_json = html.select(&m3u8_selector)
            .next()
            .ok_or_else(|| Error::ParseError("Failed to find m3u8 json msg".to_string()))?
            .text()
            .collect::<Vec<_>>()[0].split("=")
            .collect::<Vec<_>>()[1].trim().to_string();
        let msg: Value = serde_json::from_str(&m3u8_json)?;
        Ok(Uri {
            uri: msg["url"].to_string().trim_matches('\"').to_string(),
            utype: URIType::M3U8,
        })
    }
}

