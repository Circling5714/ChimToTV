using System.Windows;

namespace ConBangTV.Player;

/// Cửa sổ phủ trong suốt nằm trên trình phát (WebView2 là cửa sổ riêng của Windows, giao diện WPF không vẽ đè lên được).
/// Mọi điều khiển do PlayerView làm; cửa sổ này chỉ có các phần tử giao diện.
public partial class PlayerOverlay : Window
{
    public PlayerOverlay()
    {
        InitializeComponent();
    }
}
