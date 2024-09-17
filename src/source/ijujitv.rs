
use super::FilmInfo;

use scraper::{Html, Selector};
use url::Url;
use serde_json::Value;
use tokio::task::JoinSet;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct IJUJITV {
    url: String,
    name: String,
    release_time: String,
    genre: String,
    language: String,
    director: String,
    starring: String,
    introduction: String,
    region: String,
    sources: Vec<Vec<String>>,
}

impl Default for IJUJITV {
    fn default() -> Self {
        Self {
            url: String::new(),
            name: String::new(),
            release_time: String::new(),
            genre: String::new(),
            language: String::new(),
            director: String::new(),
            starring: String::new(),
            introduction: String::new(),
            region: String::new(),
            sources: Vec::new(),
        }
    }
}

impl FilmInfo for IJUJITV {
    fn name(&self) -> &str {
        &self.name
    }
    fn release_time(&self) -> &str {
        &self.release_time
    }
    fn genre(&self) -> &str {
        &self.genre
    }
    fn language(&self) -> &str {
        &self.language
    }
    fn director(&self) -> &str {
        &self.director
    }
    fn starring(&self) -> &str {
        &self.starring
    }
    fn introduction(&self) -> &str {
        &self.introduction
    }
    fn region(&self) -> &str {
        &self.region
    }
    fn sources(&self) -> &Vec<Vec<String>> {
        &self.sources
    }
}

impl IJUJITV {
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_string(),
            ..Default::default()
        }
    }

    pub fn from_id(id: u64) -> Self {
        Self::new(&format!("https://v.ijujitv.cc/detail/{}.html", id))
    }

    pub async fn parse(&mut self) -> Result<(), reqwest::Error> {
        let body = reqwest::get(&self.url).await?.text().await?;
        let html = Html::parse_document(&body);

        let info_dict = self.parse_info(&html);
        self.name = info_dict.get("name").unwrap().to_string();
        self.release_time = info_dict.get("年代").unwrap().to_string();
        self.genre = "-".to_string();
        self.language = info_dict.get("语言").unwrap().to_string();
        self.director = info_dict.get("导演").unwrap().to_string();
        self.starring = info_dict.get("主演").unwrap().to_string();
        self.region = "_".to_string();
        self.introduction = info_dict.get("简介").unwrap().to_string(); 

        let mut sources = self.parse_sources(&html);
        let mut tasks = JoinSet::new();
        for (i, source) in sources.iter().enumerate() {
            for (j, url) in source.iter().enumerate() {
                let url = url.to_string();
                tasks.spawn(async move {
                    (i, j, Self::parse_m3u8(&url).await)
                });
            }
        }

        while let Some(result) = tasks.join_next().await {
            let (i,j,m3u8) = result.unwrap();
            if let Ok(m3u8) = m3u8 {
                sources[i][j] = m3u8;
            } else {
                let url = sources[i][j].to_string();
                tasks.spawn(async move {
                    (i, j, Self::parse_m3u8(&url).await)
                });
            }
        }
        self.sources = sources;
        Ok(())
    }

    fn parse_info(&self, html: &Html) -> HashMap<String, String> {
        let info_selector = Selector::parse("div.albumDetailMain-right").unwrap();
        let attr_selector = Selector::parse("p:has(label)").unwrap();

        let info_div = html.select(&info_selector).next().unwrap();

        let mut info_dict: HashMap<String, String> = info_div.select(&attr_selector)
            .map(|node| node.text().collect::<String>())
            .map(|text| text.trim().split('：').map(|s| Some(s.trim().to_string())).collect::<Vec<_>>())
            .map(|mut attr| (attr[0].take().unwrap(), attr[1].take().unwrap()))
            .collect();

        let name_selector = Selector::parse("h1.title").unwrap();
        info_dict.insert("name".to_string(), info_div.select(&name_selector).next().unwrap().text().collect::<String>());
        info_dict
    }

    async fn parse_m3u8(url: &str) -> Result<String, reqwest::Error> {
        let client = reqwest::Client::new();
        let response = client.get(url).timeout(std::time::Duration::from_secs(5)).send().await?;
        let html = Html::parse_document(&response.text().await?);
        let m3u8_selector = Selector::parse("div.playBox script").unwrap();
        let m3u8_json = html.select(&m3u8_selector).next().unwrap().text()
            .collect::<Vec<_>>()[0].split("=")
            .collect::<Vec<_>>()[1].trim().to_string();
        let msg: Value = serde_json::from_str(&m3u8_json).unwrap();
        Ok(msg["url"].to_string().trim_matches('\"').to_string())
    }

    fn parse_sources(&self, html: &Html) ->Vec<Vec<String>> {
        let mut base_url = Url::parse(&self.url).unwrap();
        let mut sources: Vec<Vec<String>> = Vec::new();
        let srcs_selector = Selector::parse("div.tab-content.stui-pannel_bd.col-pd.clearfix ul").unwrap();
        let uri_selector = Selector::parse("li a").unwrap();
        let srcs = html.select(&srcs_selector);
        for src in srcs {
            let mut source: Vec<String> = Vec::new();
            let urls = src.select(&uri_selector);
            for url in urls {
                let href = url.value().attr("href").unwrap();
                if href.starts_with("//") {
                    continue;
                }
                base_url.set_path(&href);
                source.push(base_url.as_str().to_string()); 
            }
            sources.push(source);
            break;
        }
        sources
    }
}


#[tokio::test()]
async fn test_ijujitv() -> Result<(), reqwest::Error> {
    // let mut ijujitv = IJUJITV::from_id(54460);
    let mut ijujitv = IJUJITV::from_id(5139);
    ijujitv.parse().await?;
    println!("{:?}", ijujitv);
    Ok(())
}