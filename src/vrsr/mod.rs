use std::time::Duration;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::marker::PhantomData;

use url::Url;

pub mod error;
mod request;
mod parser;

pub use self::parser::zbkyyy::ZBKYYYParser as ZBKYYYParser;
pub use self::parser::ijujitv::IJUJITVParser as IJUJITVParser;
pub use self::request::RequestorBuilder as RequestorBuilder;


pub trait Request {
    async fn request(&self, url: &str) -> Result<String, self::error::Error>;
    async fn request_with_cache(&self, url: &str, cache_time: Duration) -> Result<String, self::error::Error>;
}

#[allow(unused)]
#[derive(Debug, Clone)]
pub enum URIType {
    M3U8,
    MP4,
    UNKNOWN,
}



#[allow(unused)]
#[derive(Debug, Clone)]
pub struct Uri {
    pub uri: String,
    pub utype: URIType,
}

impl Default for Uri {
    fn default() -> Self {
        Self {
            uri: String::new(),
            utype: URIType::UNKNOWN,
        }
    }
}

pub trait EpisodeParse {
    async fn parse(&self, html: &str, _requestor: Arc<impl Request>) -> Result<Uri, self::error::Error>;
}

#[derive(Debug, Clone)]
pub struct EpisodeInfo {
    name: String,
    url: String,
}

impl Default for EpisodeInfo {
    fn default() -> Self {
        Self {
            name: String::new(),
            url: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BaseEpisode<R, P> 
where 
    R: Request,
    P: EpisodeParse,
{
    info: EpisodeInfo,
    uri: Uri,
    requestor: Arc<R>,
    parser: Arc<P>
}

#[allow(unused)]
pub trait Episode<R, P> 
where 
    R: Request,
    P: EpisodeParse,
{
    fn new(info: EpisodeInfo, requester: Arc<R>, parser: Arc<P>) -> Self;
    fn name(&self) -> &str;
    fn url(&self) -> &str;
    fn uri(&self) -> Uri;
    async fn request(&mut self) -> Result<Uri, self::error::Error>;
}


impl<R, P> Episode<R, P> for BaseEpisode<R, P> 
where 
    R: Request,
    P: EpisodeParse,
{
    fn new(info: EpisodeInfo, requestor: Arc<R>, parser: Arc<P>) -> Self {
        Self {
            info,
            uri: Uri::default(), 
            requestor,
            parser
        }
    }

    fn name(&self) -> &str {
        return self.info.name.as_str();
    }    

    fn url(&self) -> &str {
        return self.info.url.as_str();
    }

    fn uri(&self) -> Uri {
        return self.uri.clone();
    }

    async fn request(&mut self) -> Result<Uri, self::error::Error> {
        let body = self.requestor.request_with_cache(
            &self.info.url, 
            Duration::new(24 * 60 * 60 * 30, 0)
        ).await?;
        self.uri = self.parser.parse(&body, self.requestor.clone()).await?;
        return Ok(self.uri.clone());
    }
}


#[derive(Debug, Clone)]
pub struct TeleplayInfo {
    pub title: String,
    pub home_page: String,
    pub release_time: Option<String>,
    pub language: Option<String>,
    pub director: Option<String>,    
    pub starring: Option<String>,
    pub introduction: Option<String>,
    pub genre: Option<String>,
    pub region: Option<String>,
    pub update_time: Option<String>,
    pub score: Option<String>,
    pub plot: Option<String>,
    pub cover: Option<String>,
}

impl Default for TeleplayInfo {
    fn default() -> Self {
        Self {
            title: String::new(),
            home_page: String::new(),
            release_time: None,
            language: None,
            director: None,
            starring: None,
            introduction: None,
            genre: None,
            region: None,
            update_time: None,
            score: None,
            plot: None,
            cover: None,
        }
    }
}

pub trait TeleplayParse {
    async fn parse(&self, html: &str, _teleplay_info: &mut TeleplayInfo, _requestor: Arc<impl Request>) -> Result<Vec<Vec<EpisodeInfo>>, self::error::Error>;
}

#[derive(Debug, Clone)]
pub struct BaseTeleplay<R, P, EP, E>
where 
    R: Request,
    P: TeleplayParse,
    EP: EpisodeParse,
    E: Episode<R, EP>,
{
    info: TeleplayInfo,
    requestor: Arc<R>,
    parser: Arc<P>,
    eparser: Arc<EP>,
    episodes: Vec<Vec<Arc<Mutex<E>>>>,
}


#[allow(unused)]
pub trait Teleplay <'a, R, P, EP>
where 
    R: Request,
    P: TeleplayParse,
    EP: EpisodeParse,
{
    type EpisodeType: Episode<R, EP> + 'a;
    fn new(info: TeleplayInfo, requester: Arc<R>, parser: Arc<P>, eparser: Arc<EP>) -> Self;
    fn title(&self) -> &str;
    fn home_page(&self) -> &str;
    fn release_time(&self) -> Option<&str>;
    fn language(&self) -> Option<&str>;
    fn director(&self) -> Option<&str>;
    fn starring(&self) -> Option<&str>;
    fn introduction(&self) -> Option<&str>;
    fn genre(&self) -> Option<&str>;
    fn region(&self) -> Option<&str>;
    fn update_time(&self) -> Option<&str>;
    fn score(&self) -> Option<&str>;
    fn plot(&self) -> Option<&str>;
    fn cover(&self) -> Option<&str>;

    fn episodes(&self) -> &Vec<Vec<Arc<Mutex<Self::EpisodeType>>>>;
    async fn request(&'a mut self) -> Result<&'a Vec<Vec<Arc<Mutex<Self::EpisodeType>>>>, self::error::Error>;
}

impl<'a, R, P, EP, T> Teleplay<'a, R, P, EP> for BaseTeleplay<R, P, EP, T> 
where 
    R: Request,
    P: TeleplayParse,
    EP: EpisodeParse, 
    T: Episode<R, EP> + 'a,
{
    type EpisodeType = T;

    fn new(info: TeleplayInfo, requestor: Arc<R>, parser: Arc<P>, eparser: Arc<EP>) -> Self {
        Self {
            info,
            requestor,
            parser,
            eparser,
            episodes: Vec::new(),
        }
    }
    fn title(&self) -> &str {
        return self.info.title.as_str();
    }
    fn home_page(&self) -> &str {   
        return self.info.home_page.as_str();
    }
    fn release_time(&self) -> Option<&str> {
        self.info.release_time
            .as_ref()
            .map(|s| s.as_str())
    }
    fn language(&self) -> Option<&str> {
        self.info.language
            .as_ref()
            .map(|s| s.as_str())
    }
    fn director(&self) -> Option<&str> {
        self.info.director
            .as_ref()
            .map(|s| s.as_str())
    }
    fn starring(&self) -> Option<&str> {
        self.info.starring
            .as_ref()
            .map(|s| s.as_str())
    }
    fn introduction(&self) -> Option<&str> {
        self.info.introduction
            .as_ref()
            .map(|s| s.as_str())
    }

    fn cover(&self) -> Option<&str> {
        self.info.cover
            .as_ref()
            .map(|s| s.as_str())
    }

    fn genre(&self) -> Option<&str> {
        self.info.genre
            .as_ref()
            .map(|s| s.as_str())
    }
    fn region(&self) -> Option<&str> {
        self.info.region
            .as_ref()
            .map(|s| s.as_str())
    }
    fn update_time(&self) -> Option<&str> {
        self.info.update_time
            .as_ref()
            .map(|s| s.as_str())
    }
    fn score(&self) -> Option<&str> {
        self.info.score
            .as_ref()
            .map(|s| s.as_str())
    }
    fn plot(&self) -> Option<&str> {
        self.info.plot
            .as_ref()
            .map(|s| s.as_str())
    }

    fn episodes(&self) -> &Vec<Vec<Arc<Mutex<Self::EpisodeType>>>> {
        return self.episodes.as_ref();
    }

    async fn request(&'a mut self) -> Result<&'a Vec<Vec<Arc<Mutex<Self::EpisodeType>>>>, self::error::Error> {
        let response = self.requestor.request_with_cache(
            &self.info.home_page, 
            Duration::new(24 * 60 * 60 * 30, 0)
        ).await?;
        let mut hub_url = Url::parse(&self.info.home_page).unwrap();
        let episodes_info = self.parser.parse(&response, &mut self.info, self.requestor.clone()).await?;
        for episode_infos in episodes_info {
            let mut episodes_list = Vec::new();
            for mut episode_info in episode_infos {
                hub_url.set_path(&episode_info.url);
                episode_info.url = hub_url.to_string();
                let episode = T::new(episode_info, self.requestor.clone(), self.eparser.clone());
                episodes_list.push(Arc::new(Mutex::new(episode)));
            }
            self.episodes.push(episodes_list);
        }
        Ok(self.episodes.as_ref())
    }
}


pub trait ResourceParse {
    async fn parse(&self, html: &str, _requestor: Arc<impl Request>) -> Result<Vec<TeleplayInfo>, self::error::Error>;
}

#[derive(Debug, Clone)]
pub struct ResourceInfo {
    host: String,
    name: String,
    search_path: String,
    search_key: String,
}

impl Default for ResourceInfo {
    fn default() -> Self {
        Self {
            host: String::new(),
            name: String::new(),
            search_path: String::new(),
            search_key: String::new(),
        }
    }
}

pub trait GenerateResourceInfo {
    fn generate(&self) -> ResourceInfo;
}

#[derive(Debug, Clone)]
pub struct BaseResource<'a, R, P, WP, EP, W>
where
    R: Request,
    P: ResourceParse,
    WP: TeleplayParse,
    EP: EpisodeParse,
    W: Teleplay<'a, R, WP, EP> + 'a,
{
    info: ResourceInfo,
    teleplays: Vec<Arc<Mutex<W>>>,
    requestor: Arc<R>,
    parser: Arc<P>,
    wparser: Arc<WP>,
    eparser: Arc<EP>,
    _marker: PhantomData<&'a ()>,
}


#[allow(unused)]
pub trait Resource<'a, R, P, WP, EP> 
where 
    R: Request,
    P: ResourceParse,
    WP: TeleplayParse,
    EP: EpisodeParse,
{
    type TeleplayType: Teleplay<'a, R, WP, EP> + 'a;
    fn new(info: ResourceInfo, requestor: Arc<R>, parser: Arc<P>, wparser: Arc<WP>, eparser: Arc<EP>) -> Self;
    fn host(&self) -> &str;
    fn name(&self) -> &str;
    fn teleplays(&self) -> &Vec<Arc<Mutex<Self::TeleplayType>>>;
    async fn search(&'a mut self, keyword: &str) -> Result<&'a Vec<Arc<Mutex<Self::TeleplayType>>>, self::error::Error>;
}

impl<'a, R, P, WP, EP, W> Resource<'a, R, P, WP, EP> for BaseResource<'a, R, P, WP, EP, W> 
where 
    R: Request ,
    P: ResourceParse,
    WP: TeleplayParse,
    EP: EpisodeParse,
    W: Teleplay<'a, R, WP, EP> + 'a,
{
    type TeleplayType = W;
    fn new(info: ResourceInfo, requestor: Arc<R>, parser: Arc<P>, wparser: Arc<WP>, eparser: Arc<EP>) -> Self {
        Self { 
            info, 
            teleplays: Vec::new(),
            requestor,
            parser,
            wparser,
            eparser,
            _marker: PhantomData::default(),
        }
    }

    fn host(&self) -> &str {
        return self.info.host.as_str();
    }

    fn name(&self) -> &str {
        return self.info.name.as_str();
    }

    fn teleplays(&self) -> &Vec<Arc<Mutex<Self::TeleplayType>>> {
        self.teleplays.as_ref()    
    }

    async fn search(&'a mut self, keyword: &str) -> Result<&'a Vec<Arc<Mutex<Self::TeleplayType>>>, self::error::Error> {
        let mut host = Url::parse(&self.info.host).unwrap();
        let mut search_url = host.join(&self.info.search_path).unwrap();
        search_url.query_pairs_mut().append_pair(&self.info.search_key, keyword);
        let respose = self.requestor.request_with_cache(
            &search_url.to_string(), 
            Duration::new(24 * 60 * 60 * 30, 0)
        ).await?;
        let teleplay_infos = self.parser.parse(&respose, self.requestor.clone()).await?;
        for mut info in teleplay_infos {
            host.set_path(&info.home_page);
            info.home_page = host.to_string();
            let teleplay = W::new(info, self.requestor.clone(), self.wparser.clone(), self.eparser.clone());
            self.teleplays.push(Arc::new(Mutex::new(teleplay)));
        }
        Ok(self.teleplays.as_ref())
    }
}

pub type GeneralEpisode<R, P> = BaseEpisode<R, P>;
pub type GeneralTeleplay<R, P> = BaseTeleplay<R, P, P, GeneralEpisode<R, P>>;
pub type GeneralResource<'a, R, P> = BaseResource<'a, R, P, P, P, GeneralTeleplay<R, P>>;

pub fn create_resource<'a, R, P> (requestor: Arc<R>, parser:Arc<P>) -> GeneralResource<'a, R, P>
where R: Request,
      P: GenerateResourceInfo + ResourceParse + TeleplayParse + EpisodeParse,
{
    GeneralResource::new(parser.generate(), requestor.clone(), parser.clone(), parser.clone(), parser.clone())
}
