<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00133">
	<sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
		[ISM-ID-00133][Error] If ISM_NSI_EO_APPLIES and attribute 
		@ism:declassException is specified and contains the tokens [25X1-EO-12951],
		[50X1-HUM], [50X2-WMD], [NATO], [AEA] or [NATO-AEA] 
		then attribute @ism:declassDate or @ism:declassEvent must NOT be specified.
		
		Human Readable: Documents under E.O. 13526 must not specify declassDate or declassEvent if 
		a declassException of 25X1-EO-12951, 50X1-HUM, 50X2-WMD, NATO, AEA or NATO-AEA is specified.
	</sch:p>
	<sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
		If ISM_NSI_EO_APPLIES, for each element which specifies 
		@ism:declassException with a value containing token [25X1-EO-12951], [50X1-HUM], [50X2-WMD], [NATO], [AEA] 
		or [NATO-AEA] this rule ensures that attributes @ism:declassDate and @ism:declassEvent are NOT specified.
	</sch:p>
	<sch:rule id="ISM-ID-00133-R1" context="*[$ISM_NSI_EO_APPLIES and util:containsAnyOfTheTokens(@ism:declassException, ('25X1-EO-12951', '50X1-HUM', '50X2-WMD', 'NATO', 'AEA', 'NATO-AEA'))]">
		<sch:assert test="not(@ism:declassDate or @ism:declassEvent)" flag="error" role="error">
			[ISM-ID-00133][Error] If ISM_NSI_EO_APPLIES and attribute 
			@ism:declassException is specified and contains the tokens [25X1-EO-12951],
			[50X1-HUM], [50X2-WMD], [NATO], [AEA] or [NATO-AEA] 
			then attribute @ism:declassDate or @ism:declassEvent must NOT be specified.
			
			Human Readable: Documents under E.O. 13526 must not specify declassDate or declassEvent if 
			a declassException of 25X1-EO-12951, 50X1-HUM, 50X2-WMD, NATO, AEA or NATO-AEA is specified.
		</sch:assert>
	</sch:rule>
</sch:pattern>