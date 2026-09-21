import Cocoa
import WebKit

@main
final class WorkBuddy2APIApp: NSApplication {
    static func main() {
        let app = WorkBuddy2APIApp()
        let wc = WindowController()
        wc.showWindow(nil)
        wc.startPoll()
        app.setActivationPolicy(.regular)
        app.run()
    }
}

final class WindowController: NSWindowController {
    private var web: WKWebView!
    private var timer: Timer?

    convenience init() {
        self.init(window: nil)
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
    }

    override init(window: NSWindow?) {
        let content = NSView(frame: NSRect(x: 0, y: 0, width: 1100, height: 760))
        let window = NSWindow(
            contentRect: content.frame,
            styleMask: [.titled, .closable, .resizable, .miniaturizable],
            backing: .buffered,
            defer: false
        )
        window.title = "WorkBuddy2API"
        window.center()
        window.identifier = NSUserInterfaceItemIdentifier("WorkBuddy2API")
        super.init(window: window)

        let config = WKWebViewConfiguration()
        web = WKWebView(frame: content.bounds, configuration: config)
        web.navigationDelegate = self
        window.contentView = content
        window.contentView!.addSubview(web)
    }

    func startPoll() {
        timer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            guard let self else { return }
            let task = URLSession.shared.dataTask(with: URL(string: "http://127.0.0.1:8787/health")!) { data, _, _ in
                guard let data, String(data: data, encoding: .utf8)?.contains("ok") == true else { return }
                DispatchQueue.main.async {
                    self.web.load(URLRequest(url: URL(string: "http://127.0.0.1:8787/admin/")!))
                    self.timer?.invalidate()
                    self.timer = nil
                }
            }
            task.resume()
        }
    }
}

extension WindowController: WKNavigationDelegate {}