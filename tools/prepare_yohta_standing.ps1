# Register approved standing music sprites on the existing 1024px stage.
Add-Type -AssemblyName System.Drawing
$packRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../assets/mascot/yohta/yohta-chibi-sitting-mohawk-pet'))
$standingSheet = [System.Drawing.Bitmap]::FromFile((Join-Path $packRoot 'artwork-sources/standing-production.png'))
function Save-StandingPart($sourceImage, $sourceBox, $destinationBox, $name) {
    $partCanvas = New-Object System.Drawing.Bitmap 1024,1024
    $partGraphics = [System.Drawing.Graphics]::FromImage($partCanvas)
    try {
        $partGraphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
        $destination = [System.Drawing.Rectangle]::new($destinationBox[0], $destinationBox[1], $destinationBox[2], $destinationBox[3])
        $partGraphics.DrawImage($sourceImage, $destination, $sourceBox[0], $sourceBox[1], $sourceBox[2], $sourceBox[3], [System.Drawing.GraphicsUnit]::Pixel)
        $partCanvas.Save((Join-Path $packRoot "music/$name.png"), [System.Drawing.Imaging.ImageFormat]::Png)
    } finally { $partGraphics.Dispose(); $partCanvas.Dispose() }
}
try {
    Save-StandingPart $standingSheet @(255,5,341,522) @(260,148,465,720) 'standing-body'
    $relaxedArm = $standingSheet.Clone([System.Drawing.Rectangle]::new(1036,190,138,240), [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    try {
        $relaxedArm.RotateFlip([System.Drawing.RotateFlipType]::RotateNoneFlipX)
        Save-StandingPart $relaxedArm @(0,0,138,240) @(520,580,98,170) 'standing-relaxed-arm'
    } finally { $relaxedArm.Dispose() }
    Save-StandingPart $standingSheet @(271,558,280,426) @(523,230,252,383) 'standing-stick-arm'
    Save-StandingPart $standingSheet @(1008,618,257,345) @(525,316,231,311) 'standing-phone-arm'
} finally { $standingSheet.Dispose() }
