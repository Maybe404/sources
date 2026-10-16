#![no_std]

mod html;
mod json;
mod net;

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, ImageRequestProvider, Manga, MangaPageResult, Page,
	Result, Source,
	alloc::{String, Vec},
	imports::net::Request,
	prelude::*,
};
use html::{ChapterPage as _, MangaPage as _, PageList as _};
use json::ApiResponse;
use net::Url;

pub const BASE_URL: &str = "https://www.twmanga.com";

/// Safety cap on chapter sub-pages fetched for a single chapter
const MAX_SUB_PAGES: i32 = 50;

struct Baozimanhua;

impl Source for Baozimanhua {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<aidoku::FilterValue>,
	) -> Result<MangaPageResult> {
		let url = Url::from_query_or_filters(query.as_deref(), page, &filters)?;

		// API request returns JSON
		if let Url::Filter { .. } = &url {
			let request = url.request()?;
			let json_data = request.data()?;
			let response: ApiResponse = serde_json::from_slice(&json_data)?;
			return Ok(response.into());
		}

		// Search and other requests return HTML
		let html = url.request()?.html()?;
		html.manga_page_result()
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details {
			let manga_page = Url::manga(manga.key.clone()).request()?.html()?;
			manga_page.update_details(&mut manga)?;
		}

		if needs_chapters {
			let chapter_page = Url::manga(manga.key.clone()).request()?.html()?;
			manga.chapters = Some(chapter_page.chapters(&manga.key)?);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let mut pages: Vec<Page> = Vec::new();
		let mut seen: Vec<String> = Vec::new();
		let mut page = 1;
		loop {
			let html = Url::chapter(manga.key.clone(), chapter.key.clone(), page)
				.request()?
				.html()?;
			// adjacent sub-pages overlap, so keep the first occurrence of each image
			for item in html.pages()? {
				if let aidoku::PageContent::Url(url, _) = &item.content {
					if seen.contains(url) {
						continue;
					}
					seen.push(url.clone());
				}
				pages.push(item);
			}
			page += 1;
			if page > MAX_SUB_PAGES || !html.has_sub_page(&manga.key, &chapter.key, page) {
				break;
			}
		}
		Ok(pages)
	}
}

impl ImageRequestProvider for Baozimanhua {
	fn get_image_request(
		&self,
		url: String,
		_context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		let url = url.replace(".baozicdn.com", ".baozimh.com");
		Ok(Request::get(url)?.header("Referer", BASE_URL))
	}
}

impl DeepLinkHandler for Baozimanhua {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		// accept links from any mirror domain by dropping the scheme and host
		let path = url
			.split_once("://")
			.map_or(url.as_str(), |(_, rest)| rest)
			.split_once('/')
			.map_or("", |(_, path)| path);
		let path = path.split(['?', '#']).next().unwrap_or_default();
		let mut splits = path.split('/');
		let deep_link_result = match splits.next() {
			Some("comic") => {
				match (splits.next(), splits.next()) {
					// /comic/{manga_id}
					(Some(manga_id), None) => Some(DeepLinkResult::Manga {
						key: manga_id.into(),
					}),
					// /comic/chapter/{manga_id}
					// /comic/chapter/{manga_id}/{section}_{chapter_id}[_{page}].html
					(Some("chapter"), Some(manga_id)) => splits
						.next()
						.and_then(|file| file.trim_end_matches(".html").split('_').nth(1))
						.map(|chapter_id| DeepLinkResult::Chapter {
							manga_key: manga_id.into(),
							key: chapter_id.into(),
						}),
					_ => None,
				}
			}
			_ => None,
		};
		Ok(deep_link_result)
	}
}

register_source!(Baozimanhua, ImageRequestProvider, DeepLinkHandler);
