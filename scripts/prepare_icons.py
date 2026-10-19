import argparse
from pathlib import Path
from PIL import Image

parser=argparse.ArgumentParser()
parser.add_argument('source',type=Path)
parser.add_argument('--desktop',type=Path)
args=parser.parse_args()
root=Path(__file__).resolve().parents[1]
image=Image.open(args.source).convert('RGBA')
image.thumbnail((512,512),Image.Resampling.LANCZOS)
canvas=Image.new('RGBA',(512,512),(0,0,0,0))
canvas.alpha_composite(image,((512-image.width)//2,(512-image.height)//2))
(root/'public').mkdir(exist_ok=True)
canvas.save(root/'public/logo.png')
icons=root/'src-tauri/icons'
icons.mkdir(parents=True,exist_ok=True)
if args.desktop:
    image=Image.open(args.desktop).convert('RGBA')
    image.thumbnail((512,512),Image.Resampling.LANCZOS)
    canvas=Image.new('RGBA',(512,512),(0,0,0,0))
    canvas.alpha_composite(image,((512-image.width)//2,(512-image.height)//2))
canvas.save(icons/'icon.png')
canvas.save(icons/'tray.png')
canvas.save(icons/'start-menu.ico',sizes=[(16,16),(24,24),(32,32),(48,48),(64,64),(128,128),(256,256)])
canvas.save(icons/'icon.ico',sizes=[(16,16),(24,24),(32,32),(48,48),(64,64),(128,128),(256,256)])
for size,name in [(32,'32x32.png'),(128,'128x128.png'),(256,'128x128@2x.png')]:
    canvas.resize((size,size),Image.Resampling.LANCZOS).save(icons/name)
print('Packaged icon alpha range:',canvas.getchannel('A').getextrema())
