using AngleSharp;
using AngleSharp.Dom;
using AngleSharp.Html.Parser;

namespace ConBangTV.Core;

/// Đọc HTML bằng AngleSharp, cùng cách gọi như jsoup / SwiftSoup của hai bản kia (q, q1, txt, at, abs).
public static class Html
{
    private static readonly IBrowsingContext Context = BrowsingContext.New(Configuration.Default);

    /// Tài liệu kèm địa chỉ gốc (để đổi đường dẫn tương đối ra tuyệt đối).
    public static IDocument Parse(string html, string baseUri) =>
        Context.OpenAsync(r => r.Content(html).Address(baseUri)).GetAwaiter().GetResult();

    public static List<IElement> Q(this IParentNode e, string css) => e.QuerySelectorAll(css).ToList();

    public static IElement? Q1(this IParentNode e, string css) => e.QuerySelector(css);

    /// Chữ của phần tử, khoảng trắng gộp lại (như text() của jsoup; bỏ chữ trong script / style).
    public static string Txt(this IElement e)
    {
        if (e.QuerySelector("script, style") == null) return e.TextContent.CollapseSpaces();
        var c = (IElement)e.Clone();
        foreach (var s in c.QuerySelectorAll("script, style").ToList()) s.Remove();
        return c.TextContent.CollapseSpaces();
    }

    public static string At(this IElement e, string key) => e.GetAttribute(key) ?? "";

    /// Đường dẫn tuyệt đối của thuộc tính ("" nếu không có thuộc tính; rỗng → địa chỉ trang, như absUrl của jsoup).
    public static string Abs(this IElement e, string key)
    {
        var v = e.GetAttribute(key);
        if (v == null) return "";
        var baseUrl = e.Owner?.BaseUrl;
        if (baseUrl == null) return v.StartsWith("http") ? v : "";
        var u = new Url(baseUrl, v.Trim());
        return u.IsInvalid ? "" : u.Href;
    }

    /// Nội dung thẻ script.
    public static string ScriptText(this IElement e) => e.TextContent;

    public static string Location(this IDocument d) => d.Url;

    public static string HtmlText(this IDocument d) => d.DocumentElement.OuterHtml;
}
