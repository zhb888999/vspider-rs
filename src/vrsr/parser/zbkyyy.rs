use super::super::{ResourceInfo, TeleplayInfo, EpisodeInfo, Uri, URIType};
use super::super::{GenerateResourceInfo, ResourceParse, TeleplayParse, EpisodeParse};
use scraper::{Html, Selector};
use std::collections::HashMap;
use super::super::error::Error;
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ZBKYYYParser {
    info: ResourceInfo,
}

impl ZBKYYYParser {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            info: ResourceInfo {
                name: "zbkyyy".to_string(),
                host: "https://www.zbkyyy.com".to_string(),
                search_path: "qyvodsearch/-------------.html".to_string(),
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

impl GenerateResourceInfo for ZBKYYYParser {
    fn generate(&self) -> ResourceInfo {
        self.info.clone()
    }
}

impl ResourceParse for ZBKYYYParser {
    fn parse(&self, html: &str) -> Result<Vec<TeleplayInfo>, Error> {
        let html = Html::parse_document(&html);

        let mut result: Vec<TeleplayInfo> = Vec::new();

        let search_selector = Selector::parse("div.intro_con").unwrap();
        let score_selector = Selector::parse("div.tit span.s_score").unwrap();
        let name_selector = Selector::parse("div.tit span.s_tit a strong").unwrap();
        let type_selector = Selector::parse("div.tit span.s_type").unwrap();
        let url_selector = Selector::parse("div.tit span.s_tit a").unwrap();
        let films = html.select(&search_selector);
        for film in films {
            let _fscore = film.select(&score_selector).next().unwrap().inner_html();
            let _ftype = film.select(&type_selector).next().unwrap().inner_html();
            let fname = film.select(&name_selector).next().unwrap().inner_html();
            let furl = film.select(&url_selector).next().unwrap().value().attr("href").unwrap();
            let mut info = TeleplayInfo::default();
            info.home_page = furl.to_string();
            info.title = fname.to_string();
            result.push(info);
        }

        Ok(result)
    }
}

impl TeleplayParse for ZBKYYYParser {
    fn parse(&self, html: &str) -> Result<Vec<Vec<EpisodeInfo>>, Error> {
        let html = Html::parse_document(&html);
        let mut sources: Vec<Vec<EpisodeInfo>> = Vec::new();
        let srcs_selector = Selector::parse("div.v_con_box ul").unwrap();
        let uri_selector = Selector::parse("li a").unwrap();
        let srcs = html.select(&srcs_selector);
        for src in srcs {
            let mut source: Vec<EpisodeInfo> = Vec::new();
            let urls = src.select(&uri_selector);
            for url in urls {
                let href = url.value().attr("href").unwrap();
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

impl EpisodeParse for ZBKYYYParser {
    fn parse(&self, html: &str) -> Result<Uri, Error> {
        let html = Html::parse_document(html);
        let m3u8_selector = Selector::parse("div.iplays script").unwrap();
        let m3u8_json = html.select(&m3u8_selector).next().unwrap().text()
            .collect::<Vec<_>>()[0].split("=")
            .collect::<Vec<_>>()[1].trim().to_string();
        let msg: Value = serde_json::from_str(&m3u8_json).unwrap();
        Ok(Uri {
            uri: msg["url"].to_string().trim_matches('\"').to_string(),
            utype: URIType::M3U8,
        })
    }
}

