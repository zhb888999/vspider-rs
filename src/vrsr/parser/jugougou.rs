use super::super::error::Error;
use super::super::{EpisodeInfo, ResourceInfo, TeleplayInfo, TeleplaySrc, URIType, Uri};
use super::super::{EpisodeParse, GenerateInfo, Request, ResourceParse, TeleplayParse};
use scraper::{ElementRef, Html, Selector};
use serde_json::Value;
use std::sync::Arc;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct JUGOUGOUParser {
    info: ResourceInfo,
}

impl Default for JUGOUGOUParser {
    fn default() -> Self {
        Self {
            info: ResourceInfo {
                name: "剧狗狗".to_string(),
                host: "https://www.jugougou.me".to_string(),
                search_path: "vodsearch/-------------.html".to_string(),
                search_key: "wd".to_string(),
            },
        }
    }
}

impl JUGOUGOUParser {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl GenerateInfo for JUGOUGOUParser {
    fn generate_resource_info(&self) -> ResourceInfo {
        self.info.clone()
    }

    fn generate_teleplay_info(&self, id: u64) -> TeleplayInfo {
        let mut host_url = url::Url::parse(&self.info.host).unwrap();
        host_url.set_path(&format!("voddetail/{}.html", id));
        TeleplayInfo {
            id,
            home_page: host_url.to_string(),
            ..TeleplayInfo::default()
        }
    }
}

impl ResourceParse for JUGOUGOUParser {
    async fn parse(
        &self,
        html: &str,
        _requestor: Arc<impl Request>,
    ) -> Result<Vec<TeleplayInfo>, Error> {
        let html = Html::parse_document(&html);
        let mut infos: Vec<TeleplayInfo> = Vec::new();

        let search_list_selector = Selector::parse("div.ewave-pannel.clearfix ul.ewave-vodlist.clearfix li.ewave-vodlist__item")?;
        let title_selector = Selector::parse("h4.ewave-vodlist__title a")?;
        let home_page_selector = Selector::parse("h4.ewave-vodlist__title a")?;
        let cover_selector = Selector::parse("a.ewave-vodlist__thumb.lazyload")?;
        let status_selector = Selector::parse("a.ewave-vodlist__thumb.lazyload span.pic-text")?;

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

            infos.push(info);
        }
        Ok(infos)
    }
}

impl TeleplayParse for JUGOUGOUParser {
    async fn parse(
        &self,
        html: &str,
        _teleplay_info: &mut TeleplayInfo,
        _requestor: Arc<impl Request>,
    ) -> Result<Vec<TeleplaySrc>, Error> {
        println!("start: {:?}", _teleplay_info);
        let html = Html::parse_document(&html);
        let elements_selector = Selector::parse("div.container div.row div.foornav+div.ewave-pannel.clearfix div.ewave-content__detail")?;
        let introduction_selector = Selector::parse("div.container div.row div.ewave-header__menu.clearfix+div.ewave-pannel.clearfix div.ewave-content div.art-content")?;
        let title_selector = Selector::parse("h3.title")?;
        let genre_selector = Selector::parse("p:nth-child(3)")?;
        let region_selector = Selector::parse("p:nth-child(4)")?;
        let release_time_selector = Selector::parse("p:nth-child(5)")?;
        let language_selector = Selector::parse("p:nth-child(6)")?;
        let update_selector = Selector::parse("p:nth-child(7)")?;

        let element_parse = |element: ElementRef, selector| -> Option<String> {
            Some(
                element
                    .select(selector)
                    .next()?
                    .last_child()?
                    .value()
                    .as_text()?
                    .to_string(),
            )
        };

        let elements = html.select(&elements_selector).next()
            .ok_or_else(|| Error::ParseError("Failed to find elements".to_string()))?;

        if let Some(title) = element_parse(elements, &title_selector) {
            if _teleplay_info.title.is_empty() {
                _teleplay_info.title = title;
            }
        }

        if let Some(genre) = element_parse(elements, &genre_selector) {
            _teleplay_info.genre.replace(genre);
        }

        if let Some(region) = element_parse(elements, &region_selector) {
            _teleplay_info.region.replace(region);
        }

        if let Some(times) = element_parse(elements, &release_time_selector) {
            _teleplay_info.release_time.replace(times);
        }

        if let Some(language) = element_parse(elements, &language_selector) {
            _teleplay_info.language.replace(language);
        }   

        if let Some(update_time) = element_parse(elements, &update_selector) {
            _teleplay_info.update_time.replace(update_time);
        }

        if let Some(introduction) = html.select(&introduction_selector).next() {
            _teleplay_info.introduction.replace(introduction.inner_html().trim().to_string());
        }
        println!("end: {:?}", _teleplay_info);

        let mut sources: Vec<TeleplaySrc> = Vec::new();
        // let srcs_name_selector = Selector::parse("div.play_source_tab.clearfix a")?;
        let srcs_selector = Selector::parse("div.container div.row div.ewave-pannel.clearfix+div.ewave-pannel.clearfix")?;
        let name_selector = Selector::parse("div.ewave-pannel__head.clearfix h3.title")?;
        let uri_selector = Selector::parse("div.ewave-content.col-pd.clearfix div.ewave-content__playlist ul li a")?;
        for src in html.select(&srcs_selector) {
            let mut source: TeleplaySrc = TeleplaySrc::new();
            source.set_name(src.select(&name_selector).next().unwrap().inner_html().trim());
            for uri in src.select(&uri_selector) {
                let info = EpisodeInfo {
                    name: uri.inner_html().trim().to_string(),
                    url: uri
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

impl EpisodeParse for JUGOUGOUParser {
    async fn parse(&self, html: &str, _requestor: Arc<impl Request>) -> Result<Uri, Error> {
        let html = Html::parse_document(html);
        let m3u8_selector = Selector::parse("div.ewave-player__video.embed-responsive script")?;
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
        println!("url: {:?}", msg["url"]);
        let mut form_data = HashMap::new();

        let vid = msg["url"]
            .as_str()
            .ok_or_else(|| {
                Error::ParseError("Faild to find url string from json msg".to_string())
            })?
            .trim_matches('"')
            .to_string();
        form_data.insert("vid".to_string(), vid);
        let res =_requestor.post_request("https://www.jugougou.me/parse/api.php", form_data).await?;
        println!("res: {:?}", res);

        Ok(Uri {
            uri: "".to_string(),
            utype: URIType::M3U8,
        })
    }
}
