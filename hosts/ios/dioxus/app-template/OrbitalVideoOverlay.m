#import <Foundation/Foundation.h>
#import <UIKit/UIKit.h>
#import <WebKit/WebKit.h>
#include <stdbool.h>

extern void orbital_native_video_event(int32_t node_id, int32_t kind,
                                       const char *source, const char *error, const char *code);

@interface OrbitalVideoOverlay : NSObject <WKNavigationDelegate, WKScriptMessageHandler>
@property(nonatomic, strong) NSMutableDictionary<NSNumber *, WKWebView *> *views;
@property(nonatomic, strong) NSMutableDictionary<NSNumber *, NSString *> *identities;
@property(nonatomic, strong) NSMutableDictionary<NSNumber *, NSString *> *sources;
+ (instancetype)shared;
- (void)upsert:(int32_t)nodeId videoId:(NSString *)videoId url:(NSString *)url html:(NSString *)html
         title:(NSString *)title autoplay:(BOOL)autoplay controls:(BOOL)controls
         start:(double)start x:(float)x y:(float)y width:(float)width height:(float)height;
- (void)remove:(int32_t)nodeId;
@end

@implementation OrbitalVideoOverlay

+ (instancetype)shared {
    static OrbitalVideoOverlay *instance;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        instance = [OrbitalVideoOverlay new];
        instance.views = [NSMutableDictionary new];
        instance.identities = [NSMutableDictionary new];
        instance.sources = [NSMutableDictionary new];
    });
    return instance;
}

- (void)upsert:(int32_t)nodeId videoId:(NSString *)videoId url:(NSString *)url html:(NSString *)html
         title:(NSString *)title autoplay:(BOOL)autoplay controls:(BOOL)controls
         start:(double)start x:(float)x y:(float)y width:(float)width height:(float)height {
    NSNumber *key = @(nodeId);
    if (width <= 0 || height <= 0) { [self remove:nodeId]; return; }
    BOOL youtube = videoId.length > 0;
    NSString *source = youtube ? videoId : url;
    if (source.length == 0) { [self remove:nodeId]; return; }
    UIWindow *window = nil;
    for (UIScene *scene in [UIApplication sharedApplication].connectedScenes) {
        if (![scene isKindOfClass:UIWindowScene.class]) continue;
        for (UIWindow *candidate in ((UIWindowScene *)scene).windows) {
            if (candidate.isKeyWindow) { window = candidate; break; }
        }
        if (window) break;
    }
    UIView *parent = window.rootViewController.view;
    if (!parent) return;
    WKWebView *web = self.views[key];
    if (!web) {
        WKWebViewConfiguration *config = [WKWebViewConfiguration new];
        config.allowsInlineMediaPlayback = YES;
        config.mediaTypesRequiringUserActionForPlayback = WKAudiovisualMediaTypeNone;
        [config.userContentController addScriptMessageHandler:self name:@"orbitalVideo"];
        web = [[WKWebView alloc] initWithFrame:CGRectZero configuration:config];
        web.navigationDelegate = self;
        web.backgroundColor = UIColor.blackColor;
        self.views[key] = web;
        [parent addSubview:web];
    }
    web.accessibilityLabel = title;
    CGFloat scale = window.screen.scale;
    web.frame = CGRectMake(x / scale, y / scale, width / scale, height / scale);
    NSString *identity = [NSString stringWithFormat:@"%c:%@:%d:%d:%g", youtube ? 'y' : 'u',
                          source, autoplay, controls, start];
    if ([identity isEqualToString:self.identities[key]]) return;
    self.identities[key] = identity;
    self.sources[key] = source;
    if (youtube) {
        [web loadHTMLString:html baseURL:[NSURL URLWithString:@"https://www.youtube.com"]];
    } else {
        NSURL *target = [NSURL URLWithString:source];
        if (target) [web loadRequest:[NSURLRequest requestWithURL:target]];
    }
}

- (void)remove:(int32_t)nodeId {
    NSNumber *key = @(nodeId);
    WKWebView *web = self.views[key];
    if (!web) return;
    [web.configuration.userContentController removeScriptMessageHandlerForName:@"orbitalVideo"];
    [web stopLoading];
    [web removeFromSuperview];
    [self.views removeObjectForKey:key];
    [self.identities removeObjectForKey:key];
    [self.sources removeObjectForKey:key];
}

- (void)webView:(WKWebView *)web didFinishNavigation:(WKNavigation *)navigation {
    for (NSNumber *key in self.views) {
        if (self.views[key] == web && ![self.identities[key] hasPrefix:@"y:"]) {
            NSString *source = self.sources[key];
            orbital_native_video_event(key.intValue, 0, source.UTF8String, "", "");
            break;
        }
    }
}

- (void)userContentController:(WKUserContentController *)controller didReceiveScriptMessage:(WKScriptMessage *)message {
    if (![message.body isKindOfClass:NSDictionary.class]) return;
    NSDictionary *body = message.body;
    NSNumber *key = body[@"node"];
    NSNumber *kind = body[@"kind"];
    NSString *source = self.sources[key];
    if (!source || !kind) return;
    NSString *code = body[@"code"] ?: @"";
    orbital_native_video_event(key.intValue, kind.intValue, source.UTF8String,
                               kind.intValue == 2 ? "Video playback failed" : "", code.UTF8String);
}

@end

void orbital_video_overlay_upsert(int32_t node_id, const char *video_id, const char *url,
                                  const char *title, const char *html,
                                  bool autoplay, bool controls, double start,
                                  float x, float y, float width, float height) {
    NSString *videoId = video_id ? [NSString stringWithUTF8String:video_id] : nil;
    NSString *pageUrl = url ? [NSString stringWithUTF8String:url] : nil;
    NSString *accessibleTitle = title ? [NSString stringWithUTF8String:title] : nil;
    NSString *pageHtml = html ? [NSString stringWithUTF8String:html] : nil;
    dispatch_async(dispatch_get_main_queue(), ^{
        [[OrbitalVideoOverlay shared] upsert:node_id
             videoId:videoId url:pageUrl html:pageHtml title:accessibleTitle
             autoplay:autoplay controls:controls start:start x:x y:y width:width height:height];
    });
}

void orbital_video_overlay_remove(int32_t node_id) {
    dispatch_async(dispatch_get_main_queue(), ^{ [[OrbitalVideoOverlay shared] remove:node_id]; });
}
