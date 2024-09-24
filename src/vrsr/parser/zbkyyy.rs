use super::super::{ResourceInfo, TeleplayInfo, EpisodeInfo, Uri, URIType};
use super::super::{GenerateInfo, ResourceParse, TeleplayParse, EpisodeParse, Request};
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
                name: "真不卡影院".to_string(),
                host: "https://www.zbkyyy.com".to_string(),
                search_path: "qyvodsearch/-------------.html".to_string(),
                search_key: "wd".to_string(),
            },
        })
    }
}

impl GenerateInfo for ZBKYYYParser {
    fn generate_resource_info(&self) -> ResourceInfo {
        self.info.clone()
    }

    fn generate_teleplay_info(&self, title: &str, id: u64) -> TeleplayInfo {
        let mut host_url = url::Url::parse(&self.info.host).unwrap();
        host_url.set_path(&format!("qyvoddetail/{}.html", id));
        TeleplayInfo {
            title: title.to_string(),
            home_page: host_url.to_string(),
            ..TeleplayInfo::default()
        }
    }
}

impl ResourceParse for ZBKYYYParser {
    async fn parse(&self, html: &str, _requestor: Arc<impl Request>) -> Result<Vec<TeleplayInfo>, Error> {
        let html = Html::parse_document(&html);
        let mut infos: Vec<TeleplayInfo> = Vec::new();

        let search_list_selector = Selector::parse("div.tv-bd.search-list div.item.clearfix")?;
        let title_selector = Selector::parse("div.item_txt div.intro_con div.tit span.s_tit a strong")?;
        let home_page_selector = Selector::parse("div.item_txt div.intro_con div.tit span.s_tit a")?;
        let score_selector = Selector::parse("div.item_txt div.intro_con div.tit span.s_score")?;
        let introduction_selector = Selector::parse("div.item_txt div.intro_con div.p_intro")?;
        let cover_selector = Selector::parse("div.item_pic img")?;
        let other_selector = Selector::parse("div.item_txt ul.txt_list.clearfix li.clearfix")?;
        let name_selector = Selector::parse("li>a")?;
        let times_lang_selector = Selector::parse("em>a")?;

        let teleplays = html.select(&search_list_selector);
        for teleplay in teleplays {
            let title = teleplay.select(&title_selector)
                .next()
                .ok_or_else(|| Error::ParseError("Failed to find name".to_string()))?
                .inner_html();
            let home_page = teleplay.select(&home_page_selector)
                .next()
                .ok_or_else(|| Error::ParseError("Failed to find home page".to_string()))?
                .value().attr("href")
                .ok_or_else(|| Error::ParseError("Failed to find home page".to_string()))?;
            let score = teleplay.select(&score_selector)
                .next()
                .ok_or_else(|| Error::ParseError("Failed to find score".to_string()))?
                .inner_html();
            let introduction = teleplay.select(&introduction_selector)
                .next()
                .ok_or_else(|| Error::ParseError("Failed to find introduction".to_string()))?
                .inner_html();
            let cover = teleplay.select(&cover_selector)
                .next()
                .ok_or_else(|| Error::ParseError("Failed to find cover".to_string()))?
                .value().attr("src")
                .ok_or_else(|| Error::ParseError("Failed to find cover".to_string()))?;

            let mut others = teleplay.select(&other_selector);
            let li0 = others.next()
                .ok_or_else(|| Error::ParseError("Failed to find other info".to_string()))?;
            let times_lang = li0.select(&times_lang_selector)
                .map(|v|v.inner_html())
                .collect::<Vec<_>>();
            let director = li0
                .select(&name_selector)
                .map(|v|v.inner_html())
                .collect::<Vec<_>>();

            let li1 = others.next()
                .ok_or_else(|| Error::ParseError("Failed to find other info".to_string()))?;
            let starring = li1.select(&name_selector)
                .map(|v|v.inner_html())
                .collect::<Vec<_>>();

            let mut info = TeleplayInfo::default();
            info.home_page = home_page.to_string();
            info.title = title.to_string();
            info.times.replace(times_lang[0].to_string());
            info.language.replace(times_lang[1].to_string());
            info.score.replace(score.to_string());
            info.cover.replace(cover.to_string());
            info.introduction.replace(introduction.to_string());
            info.director.replace(director);
            info.starring.replace(starring);
            infos.push(info);
        }
        Ok(infos)
    }
}

impl TeleplayParse for ZBKYYYParser {
    async fn parse(&self, html: &str, _teleplay_info: &mut TeleplayInfo, _requestor: Arc<impl Request>) -> Result<Vec<Vec<EpisodeInfo>>, Error> {
        let html = Html::parse_document(&html);
        let update_selector = Selector::parse("div.txt_intro_con ul.txt_list.clearfix li:nth-child(2)")?;
        _teleplay_info.update_time.replace(html.select(&update_selector)
            .next().ok_or_else(|| Error::ParseError("Failed to find update time".to_string()))?
            .last_child().ok_or_else(|| Error::ParseError("Failed to find update time".to_string()))?
            .value().as_text().ok_or_else(|| Error::ParseError("Failed to find update time".to_string()))?
            .to_string());
        let tv_bd_selector = Selector::parse("div.tv-bd")?;
        let tv_bd = html.select(&tv_bd_selector).next().unwrap();

        let region_selector = Selector::parse("p:nth-child(4)")?;
        _teleplay_info.region.replace(tv_bd.select(&region_selector)
            .next().ok_or_else(|| Error::ParseError("Failed to find region".to_string()))?
            .last_child().ok_or_else(|| Error::ParseError("Failed to find region".to_string()))?
            .value().as_text().ok_or_else(|| Error::ParseError("Failed to find region".to_string()))?
            .to_string());

        let genre_selector = Selector::parse("p:nth-child(5)")?;
        _teleplay_info.genre.replace(tv_bd.select(&genre_selector)
            .next().ok_or_else(|| Error::ParseError("Failed to find genre".to_string()))?
            .last_child().ok_or_else(|| Error::ParseError("Failed to find genre".to_string()))?
            .value().as_text().ok_or_else(|| Error::ParseError("Failed to find genre".to_string()))?
            .to_string());

        let plot_selector = Selector::parse("p:nth-child(15)")?;
        _teleplay_info.plot.replace(tv_bd.select(&plot_selector)
            .next().ok_or_else(|| Error::ParseError("Failed to find plot".to_string()))?
            .last_child().ok_or_else(|| Error::ParseError("Failed to find plot".to_string()))?
            .value().as_text().ok_or_else(|| Error::ParseError("Failed to find plot".to_string()))?
            .to_string());

        println!("info:\n{}", _teleplay_info);
        let mut sources: Vec<Vec<EpisodeInfo>> = Vec::new();
        let srcs_selector = Selector::parse("div.v_con_box ul")?;
        let uri_selector = Selector::parse("li a")?;
        let srcs = html.select(&srcs_selector);
        for src in srcs {
            let mut source: Vec<EpisodeInfo> = Vec::new();
            let urls = src.select(&uri_selector);
            for url in urls {
                let info = EpisodeInfo {
                    name: url.inner_html(),
                    url: url.value().attr("href")
                    .ok_or_else(|| Error::ParseError("Failed to find episode url".to_string()))?
                    .to_string(),
                };
                source.push(info); 
            }
            sources.push(source);
            break;
        }
        Ok(sources)
    }
}

impl EpisodeParse for ZBKYYYParser {
    async fn parse(&self, html: &str, _requestor: Arc<impl Request>) -> Result<Uri, Error> {
        let html = Html::parse_document(html);
        let m3u8_selector = Selector::parse("div.iplays script")?;
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

