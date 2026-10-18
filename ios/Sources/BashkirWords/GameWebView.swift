import SwiftUI
import WebKit
import CoreMotion
import UniformTypeIdentifiers

struct GameWebView: UIViewRepresentable {
    func makeCoordinator() -> GameCoordinator { GameCoordinator() }

    func makeUIView(context: Context) -> WKWebView {
        let config = WKWebViewConfiguration()
        config.setURLSchemeHandler(context.coordinator.resources, forURLScheme: "bashkir")
        config.userContentController.addScriptMessageHandler(context.coordinator, contentWorld: .page, name: "bashkir")
        config.websiteDataStore = .default()
        let view = WKWebView(frame: .zero, configuration: config)
        view.navigationDelegate = context.coordinator
        view.uiDelegate = context.coordinator
        view.isOpaque = false
        view.backgroundColor = UIColor(red: 0.20, green: 0.13, blue: 0.28, alpha: 1)
        view.scrollView.backgroundColor = view.backgroundColor
        view.scrollView.bounces = false
        view.scrollView.contentInsetAdjustmentBehavior = .never
        view.isInspectable = false
        context.coordinator.webView = view
        context.coordinator.observeLifecycle()
        view.load(URLRequest(url: URL(string: "bashkir://app/index.html")!))
        return view
    }

    func updateUIView(_ view: WKWebView, context: Context) {}

    static func dismantleUIView(_ view: WKWebView, coordinator: GameCoordinator) {
        coordinator.stop()
        view.configuration.userContentController.removeScriptMessageHandler(forName: "bashkir", contentWorld: .page)
        view.navigationDelegate = nil
        view.uiDelegate = nil
    }
}

enum ResourceError: Error { case invalidPath }

final class LocalResources: NSObject, WKURLSchemeHandler {
    private let root = Bundle.main.bundleURL.appendingPathComponent("Web", isDirectory: true).standardizedFileURL

    func file(_ path: String) throws -> URL {
        let relative = path.hasPrefix("/") ? String(path.dropFirst()) : path
        let url = root.appendingPathComponent(relative.isEmpty ? "index.html" : relative).standardizedFileURL
        guard url.path.hasPrefix(root.path + "/") else { throw ResourceError.invalidPath }
        return url
    }

    func webView(_ webView: WKWebView, start urlSchemeTask: WKURLSchemeTask) {
        do {
            guard let url = urlSchemeTask.request.url, url.host == "app" else { throw ResourceError.invalidPath }
            let source = try file(url.path)
            let data = try Data(contentsOf: source)
            let mime: String
            switch source.pathExtension.lowercased() {
            case "js", "mjs": mime = "text/javascript"
            case "css": mime = "text/css"
            case "html": mime = "text/html"
            default: mime = UTType(filenameExtension: source.pathExtension)?.preferredMIMEType ?? "application/octet-stream"
            }
            urlSchemeTask.didReceive(URLResponse(url: url, mimeType: mime, expectedContentLength: data.count, textEncodingName: mime.hasPrefix("text/") ? "utf-8" : nil))
            urlSchemeTask.didReceive(data)
            urlSchemeTask.didFinish()
        } catch { urlSchemeTask.didFailWithError(error) }
    }

    func webView(_ webView: WKWebView, stop urlSchemeTask: WKURLSchemeTask) {}
}

final class GameCoordinator: NSObject, WKScriptMessageHandlerWithReply, WKNavigationDelegate, WKUIDelegate {
    weak var webView: WKWebView?
    let resources = LocalResources()
    private let motion = CMMotionManager()
    private var observers: [NSObjectProtocol] = []
    private let defaults = UserDefaults.standard
    private let prefix = "BashkirWords."

    func observeLifecycle() {
        for (name, active) in [(UIApplication.didBecomeActiveNotification, true), (UIApplication.willResignActiveNotification, false)] {
            observers.append(NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in
                guard let self else { return }
                if !active { self.motion.stopDeviceMotionUpdates() }
                self.emit("bashkir-app-state", ["isActive": active])
            })
        }
    }

    func stop() {
        motion.stopDeviceMotionUpdates()
        observers.forEach { NotificationCenter.default.removeObserver($0) }
        observers.removeAll()
    }

    deinit { stop() }

    private func emit(_ name: String, _ detail: [String: Any]) {
        guard let data = try? JSONSerialization.data(withJSONObject: detail), let json = String(data: data, encoding: .utf8) else { return }
        webView?.evaluateJavaScript("window.dispatchEvent(new CustomEvent('\(name)',{detail:\(json)}))", completionHandler: nil)
    }

    private func startMotion() {
        guard motion.isDeviceMotionAvailable, UIApplication.shared.applicationState == .active else { return }
        motion.deviceMotionUpdateInterval = 1.0 / 30.0
        motion.startDeviceMotionUpdates(to: .main) { [weak self] data, _ in
            guard let data else { return }
            self?.emit("bashkir-orientation", ["beta": data.attitude.pitch * 180 / .pi, "gamma": data.attitude.roll * 180 / .pi])
        }
    }

    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage, replyHandler: @escaping (Any?, String?) -> Void) {
        guard message.frameInfo.isMainFrame, message.frameInfo.request.url?.scheme == "bashkir",
              message.frameInfo.request.url?.host == "app",
              let body = message.body as? [String: Any], let operation = body["operation"] as? String else {
            replyHandler(nil, "Invalid bridge request"); return
        }
        switch operation {
        case "smokeResult":
            guard ProcessInfo.processInfo.arguments.contains("--smoke-test"),
                  let result = body["result"] as? [String: Any],
                  let data = try? JSONSerialization.data(withJSONObject: result, options: .prettyPrinted),
                  let documents = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first else {
                replyHandler(nil, "Test mode disabled"); return
            }
            do { try data.write(to: documents.appendingPathComponent("ios-smoke.json")); replyHandler(NSNull(), nil) }
            catch { replyHandler(nil, "Could not save test result") }
        case "get", "set", "remove":
            guard let key = body["key"] as? String, key.hasPrefix("bashkir-words:"), key.count < 200 else {
                replyHandler(nil, "Invalid storage key"); return
            }
            if operation == "get" { replyHandler((defaults.string(forKey: prefix + key) as Any?) ?? NSNull(), nil) }
            else if operation == "remove" { defaults.removeObject(forKey: prefix + key); replyHandler(NSNull(), nil) }
            else if let value = body["value"] as? String, value.utf8.count <= 2_000_000 {
                defaults.set(value, forKey: prefix + key); replyHandler(NSNull(), nil)
            } else { replyHandler(nil, "Invalid save data") }
        case "readResource":
            do {
                guard let path = body["path"] as? String, ["json", "md"].contains(URL(fileURLWithPath: path).pathExtension) else { throw ResourceError.invalidPath }
                replyHandler(try String(contentsOf: resources.file(path), encoding: .utf8), nil)
            } catch { replyHandler(nil, "Resource unavailable") }
        case "haptic":
            UIImpactFeedbackGenerator(style: .light).impactOccurred(); replyHandler(NSNull(), nil)
        case "motionStart":
            startMotion(); replyHandler(NSNull(), nil)
        case "motionStop":
            motion.stopDeviceMotionUpdates(); replyHandler(NSNull(), nil)
        default: replyHandler(nil, "Unsupported operation")
        }
    }

    private func openExternal(_ url: URL) {
        guard ["https", "http"].contains(url.scheme?.lowercased() ?? "") else { return }
        UIApplication.shared.open(url)
    }

    func webView(_ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction, decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
        if let url = navigationAction.request.url, url.scheme == "bashkir", url.host == "app" { decisionHandler(.allow) }
        else {
            if let url = navigationAction.request.url, navigationAction.navigationType == .linkActivated { openExternal(url) }
            decisionHandler(.cancel)
        }
    }

    func webView(_ webView: WKWebView, createWebViewWith configuration: WKWebViewConfiguration, for navigationAction: WKNavigationAction, windowFeatures: WKWindowFeatures) -> WKWebView? {
        if let url = navigationAction.request.url { openExternal(url) }
        return nil
    }

    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) { webView.reload() }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        #if targetEnvironment(simulator)
        if ProcessInfo.processInfo.arguments.contains("--smoke-test"),
           let file = Bundle.main.url(forResource: "SmokeTest", withExtension: "js"),
           let script = try? String(contentsOf: file, encoding: .utf8) {
            webView.evaluateJavaScript(script, completionHandler: nil)
        }
        #endif
    }
}
