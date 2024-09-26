use super::super::{ResourceInfo, TeleplayInfo, EpisodeInfo, Uri, URIType};
use super::super::{GenerateInfo, ResourceParse, TeleplayParse, EpisodeParse, Request};
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
}

impl GenerateInfo for IJUJITVParser {
    fn generate_resource_info(&self) -> ResourceInfo {
        self.info.clone()
    }

    fn generate_teleplay_info(&self, title: &str, id: u64) -> TeleplayInfo {
        let mut host_url = url::Url::parse(&self.info.host).unwrap();
        host_url.set_path(&format!("detail/{}.html", id));
        TeleplayInfo {
            title: title.to_string(),
            home_page: host_url.to_string(),
            ..TeleplayInfo::default()
        }
    }
}

impl ResourceParse for IJUJITVParser {
    async fn parse(&self, html: &str, _requestor: Arc<impl Request>) -> Result<Vec<TeleplayInfo>, Error> {
        let html = Html::parse_document(&html);
        let mut infos: Vec<TeleplayInfo> = Vec::new();

        let search_list_selector = Selector::parse("div.m-list-inner ul.m-list li.m-item")?;
        let teleplays = html.select(&search_list_selector);
        let a_selector = Selector::parse("a.thumb")?;
        let img_selector = Selector::parse("a.thumb img")?;
        let status_selector = Selector::parse("a.thumb div.icon-br span.label")?;
        let starring_selector = Selector::parse("div.text p.des")?;
        for teleplay in teleplays {
            let a = teleplay.select(&a_selector)
               .next()
               .ok_or_else(|| Error::ParseError("Failed to find title".to_string()))?;
            let title = a.value().attr("title");
            let home_page = a.value().attr("href");
            let cover = teleplay.select(&img_selector)
               .next()
               .ok_or_else(|| Error::ParseError("Failed to find cover".to_string()))?
               .value().attr("src");
            let status = teleplay.select(&status_selector)
               .next()
               .ok_or_else(|| Error::ParseError("Failed to find status".to_string()))?
               .inner_html();
            let starring = teleplay.select(&starring_selector)
                           .next()
                           .ok_or_else(|| Error::ParseError("Failed to find director".to_string()))?
                           .inner_html()
                           .split(':')
                           .skip(1)
                           .map(|v| v.trim().to_string())
                           .last();
            let mut info = TeleplayInfo::default();
            info.title = title
                .ok_or_else(|| Error::ParseError("Failed to find title".to_string()))?
                .to_string();
            info.home_page = home_page
                .ok_or_else(|| Error::ParseError("Failed to find home page".to_string()))?
                .to_string();
            info.cover.replace(cover
                .ok_or_else(|| Error::ParseError("Failed to find cover".to_string()))?
                .to_string());
            if let Some(starring) = starring {
                let names = starring.trim().split(',').map(|v| v.trim().to_string()).collect::<Vec<_>>();
                if names.len() > 1 {
                    info.starring.replace(names);
                } else {
                    info.starring.replace(names[0].trim().split(' ').map(|v| v.trim().to_string()).collect::<Vec<_>>());
                }
            }
            info.status.replace(status.trim().to_string());
            println!("{}", info);
            infos.push(info);
        }
        Ok(infos)
    }
}

impl TeleplayParse for IJUJITVParser {
    async fn parse(&self, html: &str, _teleplay_info: &mut TeleplayInfo, _requestor: Arc<impl Request>) -> Result<Vec<Vec<EpisodeInfo>>, Error> {
        let html = Html::parse_document(&html);

        let detail_selector = Selector::parse("div.albumDetailMain-right")?;
        let times_selector = Selector::parse("div.intro.clearfix p:nth-child(2) a")?;
        let lanaguage_selector = Selector::parse("div.intro.clearfix p:nth-child(3)")?;
        let director_selector = Selector::parse("div.intro.clearfix p:nth-child(4) a")?;
        let update_selector = Selector::parse("div.intro.clearfix p:nth-child(6)")?;
        let introduction_selector = Selector::parse("p.intro-desc.item-desc-info")?;

        let detail = html.select(&detail_selector)
            .next()
            .ok_or_else(|| Error::ParseError("Failed to find detail".to_string()))?;
        
        let times = detail.select(&times_selector)
            .next().ok_or_else(|| Error::ParseError("Failed to find times".to_string()))?
           .inner_html();
        _teleplay_info.times.replace(times.trim().to_string());
        let language = detail.select(&lanaguage_selector)
            .next().ok_or_else(|| Error::ParseError("Failed to find language".to_string()))?
            .last_child().ok_or_else(|| Error::ParseError("Failed to find language".to_string()))?
            .value().as_text().ok_or_else(|| Error::ParseError("Failed to find language".to_string()))?
            .to_string();
        _teleplay_info.language.replace(language.trim().to_string());
        _teleplay_info.director.replace(detail.select(&director_selector)
            .map(|v| v.inner_html().trim().to_string())
            .collect());
        let update = detail.select(&update_selector)
            .next().ok_or_else(|| Error::ParseError("Failed to find update".to_string()))?
            .last_child().ok_or_else(|| Error::ParseError("Failed to find update".to_string()))?
            .value().as_text().ok_or_else(|| Error::ParseError("Failed to find update".to_string()))?
            .trim().to_string();
        _teleplay_info.update_time.replace(update);
        let introduction = detail.select(&introduction_selector)
            .next().ok_or_else(|| Error::ParseError("Failed to find introduction".to_string()))?
            .last_child().ok_or_else(|| Error::ParseError("Failed to find introduction".to_string()))?
            .value().as_text().ok_or_else(|| Error::ParseError("Failed to find introduction".to_string()))?
            .trim().to_string();
        _teleplay_info.introduction.replace(introduction);

        println!("{:#?}\n", _teleplay_info);

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
                info.name = url.inner_html().trim().to_string();
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

