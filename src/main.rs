use dotenv::dotenv;
use lemmy_wikibot_rs::apis::lemmy_api::LemmyClient;
use lemmy_wikibot_rs::apis::wikipedia_api::get_wiki_page;
use lemmy_wikibot_rs::comment_builder;
use lemmy_wikibot_rs::{load_db, save_to_db};
use regex::Regex;
use reqwest::StatusCode;
use std::env;
use std::thread::sleep;
use std::time::Duration;

fn main() {
    dotenv().unwrap();

    let (username_or_email, password, instance, community) = (
        env::var("LEMMY_USERNAME_OR_EMAIL")
            .expect("LEMMY_USERNAME_OR_EMAIL not configured in .env"),
        env::var("LEMMY_PASSWORD").expect("LEMMY_PASSWORD not configured in .env"),
        env::var("LEMMY_INSTANCE").expect("LEMMY_INSTANCE not configured in .env"),
        env::var("LEMMY_COMMUNITY").expect("LEMMY_COMMUNITY not configured in .env"),
    );

    // login to lemmy client
    let mut client = LemmyClient::new(username_or_email, password, instance, community);
    client.login();

    loop {
        // get posts and filter out posts that are locked
        println!("Getting posts");
        let post_list_resp = match client.get_posts("NewComments", "10") {
            Ok(resp) => resp,
            Err(err) => {
                if err.is_status() {
                    match err.status().unwrap() {
                        StatusCode::TOO_MANY_REQUESTS | StatusCode::GATEWAY_TIMEOUT => {
                            sleep(Duration::new(5, 0));
                            continue;
                        },
                        StatusCode::INTERNAL_SERVER_ERROR => panic!("INTERNAL_SERVER_ERROR"),
                        _ => panic!("Unexpected status code: {}", err),
                    }
                } else {
                    panic!("Unexpected error occurred: {}", err);
                }
            }
        };
        sleep(Duration::new(3, 0));
        for post_view in post_list_resp.posts {
            let post = post_view.post;
            if post.locked {
                continue;
            } else {
                println!("Getting comments");
                let comment_list_resp =
                    match client.get_comments(post.id.to_string().as_str(), "New") {
                        Ok(resp) => resp,
                        Err(err) => {
                            if err.is_status() {
                                match err.status().unwrap() {
                                    StatusCode::TOO_MANY_REQUESTS | StatusCode::GATEWAY_TIMEOUT => {
                                        sleep(Duration::new(5, 0));
                                        continue;
                                    },
                                    StatusCode::INTERNAL_SERVER_ERROR => panic!("INTERNAL_SERVER_ERROR"),
                                    _ => panic!("Unexpected status code: {}", err),
                                }
                            } else {
                                panic!("Unexpected error occurred: {}", err);
                            }
                        }
                    };
                sleep(Duration::new(2, 0));
                for comment_view in comment_list_resp.comments {
                    let checked_comments: Vec<u32> = load_db();
                    let comment = comment_view.comment;
                    if checked_comments.contains(&comment.id) {
                        continue;
                    } else {
                        save_to_db(comment.id);

                        let re = Regex::new(r"wikipedia.org/wiki/([0-9\w_()~\-%&$,]+)").unwrap();
                        let title = match re.captures(&comment.content) {
                            Some(caps) => caps.get(1).unwrap().as_str(),
                            None => continue,
                        };
                        let wiki_page = match get_wiki_page(title.replace("))", ")")) {
                            Some(wiki_page) => wiki_page,
                            None => continue,
                        };
                        let built_comment = comment_builder(&wiki_page.title, &wiki_page.summary);
                        match client.create_comment(post.id, comment.id, built_comment.as_str()) {
                            Ok(_) => println!("Answered comment: {}", comment.id),
                            Err(err) => {
                                if err.is_status() {
                                    match err.status().unwrap() {
                                        StatusCode::TOO_MANY_REQUESTS | StatusCode::GATEWAY_TIMEOUT => {
                                            sleep(Duration::new(5, 0));
                                            continue;
                                        },
                                        StatusCode::INTERNAL_SERVER_ERROR => panic!("INTERNAL_SERVER_ERROR"),
                                        _ => panic!("Unexpected status code: {}", err),
                                    }
                                } else {
                                    panic!("Unexpected error occurred: {}", err);
                                }
                            }
                        }
                        sleep(Duration::new(1, 0));
                    }
                }
            }
        }
        sleep(Duration::new(10, 0));
    }
}
