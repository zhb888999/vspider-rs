use super::super::error::Error;
use super::super::{EpisodeInfo, ResourceInfo, TeleplayInfo, TeleplaySrc, URIType, Uri};
use super::super::{EpisodeParse, GenerateInfo, Request, ResourceParse, TeleplayParse};
use scraper::{ElementRef, Html, Selector};
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ZBKYYYParser {
    info: ResourceInfo,
}

impl Default for ZBKYYYParser {
    fn default() -> Self {
        Self {
            info: ResourceInfo {
                name: "真不卡影院".to_string(),
                host: "https://www.zbkyyy.com".to_string(),
                search_path: "qyvodsearch/-------------.html".to_string(),
                search_key: "wd".to_string(),
            },
        }
    }
}

impl ZBKYYYParser {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl GenerateInfo for ZBKYYYParser {
    fn generate_resource_info(&self) -> ResourceInfo {
        self.info.clone()
    }

    fn generate_teleplay_info(&self, id: u64) -> TeleplayInfo {
        let mut host_url = url::Url::parse(&self.info.host).unwrap();
        host_url.set_path(&format!("qyvoddetail/{}.html", id));
        TeleplayInfo {
            home_page: host_url.to_string(),
            ..TeleplayInfo::default()
        }
    }
}

impl ResourceParse for ZBKYYYParser {
    async fn parse(
        &self,
        html: &str,
        _requestor: Arc<impl Request>,
    ) -> Result<Vec<TeleplayInfo>, Error> {
        let html = Html::parse_document(&html);
        let mut infos: Vec<TeleplayInfo> = Vec::new();

        let search_list_selector = Selector::parse("div.tv-bd.search-list div.item.clearfix")?;
        let title_selector =
            Selector::parse("div.item_txt div.intro_con div.tit span.s_tit a strong")?;
        let home_page_selector =
            Selector::parse("div.item_txt div.intro_con div.tit span.s_tit a")?;
        let score_selector = Selector::parse("div.item_txt div.intro_con div.tit span.s_score")?;
        let introduction_selector = Selector::parse("div.item_txt div.intro_con div.p_intro")?;
        let cover_selector = Selector::parse("div.item_pic img")?;
        let status_selector = Selector::parse("div.item_pic span.v-tips em")?;
        let other_selector = Selector::parse("div.item_txt ul.txt_list.clearfix li.clearfix")?;
        let name_selector = Selector::parse("li>a")?;
        let times_lang_selector = Selector::parse("em>a")?;

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
            if let Some(score) = teleplay.select(&score_selector).next() {
                info.score.replace(score.inner_html());
            }
            if let Some(introduction) = teleplay.select(&introduction_selector).next() {
                info.introduction.replace(introduction.inner_html());
            }
            if let Some(cover) = teleplay.select(&cover_selector).next() {
                if let Some(cover) = cover.value().attr("src") {
                    info.cover.replace(cover.to_string());
                }
            }
            if let Some(status) = teleplay.select(&status_selector).next() {
                info.status.replace(status.inner_html());
            }

            let mut others = teleplay.select(&other_selector);
            if let Some(li0) = others.next() {
                let times_lang = li0
                    .select(&times_lang_selector)
                    .map(|v| v.inner_html())
                    .collect::<Vec<_>>();
                if times_lang.len() == 2 {
                    info.times.replace(times_lang[0].to_string());
                    info.language.replace(times_lang[1].to_string());
                }
                let director = li0
                    .select(&name_selector)
                    .map(|v| v.inner_html())
                    .collect::<Vec<_>>();
                if !director.is_empty() {
                    info.director.replace(director);
                }
            }
            if let Some(li1) = others.next() {
                let starring = li1
                    .select(&name_selector)
                    .map(|v| v.inner_html())
                    .collect::<Vec<_>>();
                if !starring.is_empty() {
                    info.starring.replace(starring);
                }
            }
            infos.push(info);
        }
        Ok(infos)
    }
}

impl TeleplayParse for ZBKYYYParser {
    async fn parse(
        &self,
        html: &str,
        _teleplay_info: &mut TeleplayInfo,
        _requestor: Arc<impl Request>,
    ) -> Result<Vec<TeleplaySrc>, Error> {
        let html = Html::parse_document(&html);
        let update_selector =
            Selector::parse("div.txt_intro_con ul.txt_list.clearfix li:nth-child(2)")?;

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

        if let Some(update_time) = element_parse(html.root_element(), &update_selector) {
            _teleplay_info.update_time.replace(update_time);
        }

        let tv_bd_selector = Selector::parse("div.tv-bd")?;
        if let Some(tv_bd) = html.select(&tv_bd_selector).next() {
            let title_selector = Selector::parse("p:nth-child(1)")?;
            if let Some(tilte) = element_parse(tv_bd, &title_selector) {
                if _teleplay_info.title.is_empty() {
                    _teleplay_info.title = tilte;
                }
            }
            let region_selector = Selector::parse("p:nth-child(4)")?;
            if let Some(region) = element_parse(tv_bd, &region_selector) {
                _teleplay_info.region.replace(region);
            }
            let genre_selector = Selector::parse("p:nth-child(5)")?;
            if let Some(genre) = element_parse(tv_bd, &genre_selector) {
                _teleplay_info.genre.replace(genre);
            }
            let plot_selector = Selector::parse("p:nth-child(15)")?;
            if let Some(plot) = element_parse(tv_bd, &plot_selector) {
                _teleplay_info.plot.replace(plot);
            }
        }
        let mut sources: Vec<TeleplaySrc> = Vec::new();
        let srcs_name_selector = Selector::parse("div.play_source_tab.clearfix a")?;
        let srcs_selector = Selector::parse("div.v_con_box ul")?;
        let uri_selector = Selector::parse("li a")?;
        let srcs = html.select(&srcs_selector);
        let srcs_name = html.select(&srcs_name_selector);
        for (src, name) in srcs.zip(srcs_name) {
            let mut source: TeleplaySrc = TeleplaySrc::new();
            source.set_name(name.inner_html().trim());
            let urls = src.select(&uri_selector);
            for url in urls {
                let info = EpisodeInfo {
                    name: url.inner_html().trim().to_string(),
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

impl EpisodeParse for ZBKYYYParser {
    async fn parse(&self, html: &str, _requestor: Arc<impl Request>) -> Result<Uri, Error> {
        let html = Html::parse_document(html);
        let m3u8_selector = Selector::parse("div.iplays script")?;
        let m3u8_json = html
            .select(&m3u8_selector)
            .next()
            .ok_or_else(|| Error::ParseError("Failed to find m3u8 json msg".to_string()))?
            .inner_html()
            .split("=")
            .last()
            .ok_or_else(|| Error::ParseError("Failed to find m3u8 json msg".to_string()))?
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
