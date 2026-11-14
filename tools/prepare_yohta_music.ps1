# Register the generated music artwork on Yohta's existing 1024px stage.
# Run from the repository root. Original art remains in artwork-sources.
Add-Type -AssemblyName System.Drawing
$packRoot = Join-Path $PSScriptRoot '../assets/mascot/yohta/yohta-chibi-sitting-mohawk-pet'
$packRoot = [System.IO.Path]::GetFullPath($packRoot)
$sheetArt = [System.Drawing.Image]::FromFile((Join-Path $packRoot 'artwork-sources/music-parts.png'))
$bodyArt = [System.Drawing.Image]::FromFile((Join-Path $packRoot 'artwork-sources/body-without-front-arms.png'))
$pawArt = [System.Drawing.Bitmap]::FromFile((Join-Path $packRoot 'artwork-sources/music-paws-v2.png'))

function Save-RegisteredPart($sourceImage, $sourceBox, $destinationBox, $name, $opacity = 1.0) {
    $partCanvas = New-Object System.Drawing.Bitmap 1024,1024
    $partGraphics = [System.Drawing.Graphics]::FromImage($partCanvas)
    $partGraphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $destination = [System.Drawing.Rectangle]::new($destinationBox[0], $destinationBox[1], $destinationBox[2], $destinationBox[3])
    $attributes = New-Object System.Drawing.Imaging.ImageAttributes
    $matrix = New-Object System.Drawing.Imaging.ColorMatrix
    $matrix.Matrix33 = $opacity
    $attributes.SetColorMatrix($matrix)
    $partGraphics.DrawImage($sourceImage, $destination, $sourceBox[0], $sourceBox[1], $sourceBox[2], $sourceBox[3], [System.Drawing.GraphicsUnit]::Pixel, $attributes)
    $partCanvas.Save((Join-Path $packRoot "music/$name.png"), [System.Drawing.Imaging.ImageFormat]::Png)
    $attributes.Dispose()
    $partGraphics.Dispose()
    $partCanvas.Dispose()
}

try {
    Save-RegisteredPart $bodyArt @(314,679,440,382) @(261,558,349,305) 'body'
    Save-RegisteredPart $pawArt @(214,94,253,503) @(334,637,104,224) 'left_arm'
    Save-RegisteredPart $pawArt @(785,94,254,503) @(443,637,104,224) 'right_arm'
    Save-RegisteredPart $pawArt @(129,706,345,463) @(444,640,124,166) 'holding_arm'
    # Stack the small curled fingertips vertically along the item handle.
    $gripArt = $pawArt.Clone([System.Drawing.Rectangle]::new(799,881,280,165), [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    try {
        $gripArt.RotateFlip([System.Drawing.RotateFlipType]::Rotate90FlipNone)
        Save-RegisteredPart $gripArt @(0,0,165,280) @(537,746,24,38) 'grip'
    } finally {
        $gripArt.Dispose()
    }
    Save-RegisteredPart $sheetArt @(125,514,193,311) @(515,689,48,80) 'phone'
    Save-RegisteredPart $sheetArt @(618,493,92,352) @(529,670,22,99) 'glow_stick'
    Save-RegisteredPart $sheetArt @(947,465,351,422) @(504,678,70,101) 'phone_glow' 0.28
    Save-RegisteredPart $sheetArt @(1439,464,221,420) @(518,659,46,120) 'stick_glow' 0.32
    $editorFolder = Join-Path $packRoot 'editor/sources/music'
    New-Item -ItemType Directory -Force $editorFolder | Out-Null
    $registeredBody = [System.Drawing.Bitmap]::FromFile((Join-Path $packRoot 'music/body.png'))
    try {
        $croppedBody = $registeredBody.Clone([System.Drawing.Rectangle]::new(261,558,349,305), [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
        try {
            $croppedBody.Save((Join-Path $editorFolder 'body.png'), [System.Drawing.Imaging.ImageFormat]::Png)
        } finally {
            $croppedBody.Dispose()
        }
    } finally {
        $registeredBody.Dispose()
    }
} finally {
    $sheetArt.Dispose()
    $bodyArt.Dispose()
    $pawArt.Dispose()
}
