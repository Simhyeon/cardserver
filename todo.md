# Make web socket server complete
* [x] Refactor model of connection into composition of users struct.
* [x] Eanble other client to connect to existing room with given id
* [x] Refactor model of ServerResponse to include simple card or non-string type 
Conclusion: refactored to include enumerator varaible.
* [x] Refactor recieve_player_action to get request not only request's action.
* [ ] Make game complete as a text based without unity's gui.
	* [x] Player can poll cards
	* [x] Player can bet with raise or call.
	* [x] Player can fold.
	* [ ] Player can win.
* [ ] Alert opponent when player gets disconnected.
	* [ ] Make reload method for User or completely clean up room.
