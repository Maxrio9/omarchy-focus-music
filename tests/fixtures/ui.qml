import QtQuick
import QtTest
import Quickshell
import Quickshell.Wayland
import qs.Commons
ShellRoot {
 QtObject {
  id: mockBar
  property bool vertical: false
  property string position: "top"
  property int barSize: 35
  property color barForeground: Color.foreground
  property color foreground: Color.foreground
  property color urgent: Color.urgent
  property string fontFamily: Style.font.family
  property bool foregroundAnimationEnabled: false
  property var activePopout: null
  function registerClickTarget(target) {}
  function unregisterClickTarget(target) {}
  function showTooltip(target,text) {}
  function hideTooltip(target) {}
  function requestPopout(owner) { activePopout=owner }
  function releasePopout(owner) { activePopout=null }
 }
 PanelWindow {
  visible:true
  screen:Quickshell.screens.find(s=>s.name==="HDMI-A-1") || Quickshell.screens[0]
  anchors { top:true;left:true }
  margins { top:120;left:1600 }
  exclusionMode:ExclusionMode.Ignore
  WlrLayershell.namespace:"focus-rust-native-test"
  implicitWidth:700;implicitHeight:600
  color:Color.background
  Loader {
   id:plugin;x:220;y:20
   source:FOCUS_WIDGET_SOURCE
   width:item?item.implicitWidth:0;height:item?item.implicitHeight:0
   onLoaded:{item.bar=mockBar;item.popupPinned=true;item.popupOpen=true}
  }
 }
 Timer { interval:1000;running:true;onTriggered:{try{checks.check()}catch(e){console.error(e.stack);Qt.quit()}} }
 TestCase {
  id:checks;when:false
  function check() {
   tryVerify(function(){return plugin.item.mediaStatusAt > 0 && plugin.item.player.phase === "Playing"},3000)
   var popup=findChild(plugin.item,"musicPopup")
   var surfaceHeight=popup.implicitHeight
   var surfaceWidth=popup.implicitWidth
   verify(surfaceHeight<popup.fittedContentHeight(Style.space(320)))
   var savedPlayer=plugin.item.player
   plugin.item.player=Object.assign({},savedPlayer,{banner:Array(100).fill("Long artist and track name").join(" ")})
   wait(80)
   compare(popup.implicitHeight,surfaceHeight)
   compare(popup.implicitWidth,surfaceWidth)
   verify(findChild(plugin.item,"popupTrackTitle").lineCount<=2)
   plugin.item.player=savedPlayer
   var genre=findChild(plugin.item,"genreSelector")
   var selectorHeight=genre.implicitHeight
   var savedOptions=genre.options
   var manyOptions=[]
   for(var n=0;n<40;n++)manyOptions.push({value:"piano",label:"Local piano collection "+n})
   genre.options=manyOptions
   genre.open()
   wait(80)
   compare(popup.implicitHeight,surfaceHeight)
   compare(genre.implicitHeight,selectorHeight)
   var list=findChild(genre,"genreList")
   verify(list.contentHeight>list.height)
   list.positionViewAtEnd()
   wait(80)
   verify(list.contentY>0)
   verify(list.mapToItem(genre.popupViewport,0,list.height).y<=genre.popupViewport.height)
   genre.close()
   genre.options=savedOptions
   var previous=findChild(plugin.item,"previousTrack")
   var next=findChild(plugin.item,"nextTrack")
   var toggle=findChild(plugin.item,"playbackToggle")
   var initial=plugin.item.player.track_id
   mouseClick(previous,previous.width/2,previous.height/2)
   tryVerify(function(){return plugin.item.player.track_id!==initial},3000)
   mouseClick(next,next.width/2,next.height/2)
   tryVerify(function(){return plugin.item.player.track_id===initial},3000)
   mouseClick(toggle,toggle.width/2,toggle.height/2)
   compare(toggle.text,"Play")
   tryVerify(function(){return plugin.item.playbackPhase==="Paused" && plugin.item.playbackOverride===""},3000)
   mouseClick(toggle,toggle.width/2,toggle.height/2)
   compare(toggle.text,"Pause")
   tryVerify(function(){return plugin.item.player.phase==="Playing"},3000)
   for(var i=0;i<11;i++)plugin.item.adjustVolume(5)
   compare(plugin.item.displayedVolume,80)
   tryVerify(function(){return plugin.item.player.volume===80 && plugin.item.displayedVolume===80},3000)
   var genreClicks=0
   genre.changed.connect(function(){genreClicks++})
   genre.options=manyOptions
   for(var last=38;last<40;last++) {
    genre.open()
    wait(50)
    genre.listView.positionViewAtIndex(last,ListView.Contain)
    wait(50)
    var row=genre.listView.itemAtIndex(last)
    verify(row!==null)
    mouseClick(row,row.width/2,row.height/2)
    tryVerify(function(){return genreClicks===last-37},1000)
    compare(genre.popupOpen,false)
   }
   genre.options=savedOptions
   var gear=findChild(plugin.item,"settingsButton")
   mouseClick(gear,gear.width/2,gear.height/2)
   compare(plugin.item.settingsOpen,true)
   compare(popup.implicitHeight,surfaceHeight)
   verify(findChild(plugin.item,"placementSetting")===null)
   var display=findChild(plugin.item,"displaySetting")
   compare(display.options[0].label,"Banner")
   compare(display.options[2].label,"Note")
   display.changed("visualizer")
   compare(findChild(plugin.item,"popupSpectrum").visible,false)
   display.open()
   wait(80)
   compare(popup.implicitHeight,surfaceHeight)
   display.close()
   plugin.item.savePreference("visualizerWidth",120)
   compare(findChild(plugin.item,"barSpectrum").barCount,6)
   compare(findChild(plugin.item,"popupSpectrum").barCount,5)
   display.changed("icon")
   compare(findChild(plugin.item,"popupSpectrum").visible,true)
   tryVerify(function(){return plugin.item.player.preferences.barDisplay==="icon" && plugin.item.player.preferences.visualizerWidth===120},4000)
   var media=findChild(plugin.item,"otherMediaSetting")
   for(var last=2;last<4;last++) {
    media.open()
    wait(50)
    verify(media.listView.mapToItem(media,0,0).y>=media.height)
    media.listView.positionViewAtIndex(last,ListView.Contain)
    wait(50)
    var row=media.listView.itemAtIndex(last)
    verify(row!==null)
    mouseClick(row,row.width/2,row.height/2)
    compare(media.popupOpen,false)
    compare(plugin.item.preferences.otherMedia,media.options[last].value)
   }
   tryVerify(function(){return plugin.item.player.preferences.otherMedia==="none"},3000)
   console.log("PASS native UI")
   Qt.quit()
  }
 }
}
