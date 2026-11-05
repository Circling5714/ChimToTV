#import "WebKitSPI.h"

// Khai báo các hàm có sẵn trong WebKit nhưng không có trong header công khai (_WKWebsiteDataStoreConfiguration,
// -[WKWebsiteDataStore _initWithConfiguration:]). Lớp lấy bằng NSClassFromString nên app vẫn chạy khi thiếu.
@interface NSObject (CBTWebKitSPI)
- (instancetype)initNonPersistentConfiguration;
- (void)setProxyConfiguration:(NSDictionary *)configuration;
- (instancetype)_initWithConfiguration:(id)configuration;
@end

static Class CBTConfigClass(void) { return NSClassFromString(@"_WKWebsiteDataStoreConfiguration"); }

NSDictionary<NSString *, NSNumber *> *CBTWebKitSPIInfo(void) {
    Class c = CBTConfigClass();
    return @{
        @"configClass": @(c != nil),
        @"initNonPersistentConfiguration": @([c instancesRespondToSelector:@selector(initNonPersistentConfiguration)]),
        @"setProxyConfiguration": @([c instancesRespondToSelector:@selector(setProxyConfiguration:)]),
        @"initWithConfiguration": @([WKWebsiteDataStore instancesRespondToSelector:@selector(_initWithConfiguration:)]),
    };
}

WKWebsiteDataStore *CBTProxiedDataStore(NSDictionary *proxy) {
    Class c = CBTConfigClass();
    if (!c || ![c instancesRespondToSelector:@selector(initNonPersistentConfiguration)] ||
        ![c instancesRespondToSelector:@selector(setProxyConfiguration:)] ||
        ![WKWebsiteDataStore instancesRespondToSelector:@selector(_initWithConfiguration:)]) {
        return nil;
    }
    @try {
        id config = [[c alloc] initNonPersistentConfiguration];
        [config setProxyConfiguration:proxy];
        return [[WKWebsiteDataStore alloc] _initWithConfiguration:config];
    } @catch (NSException *e) {
        NSLog(@"CBTProxiedDataStore: %@", e);
        return nil;
    }
}
