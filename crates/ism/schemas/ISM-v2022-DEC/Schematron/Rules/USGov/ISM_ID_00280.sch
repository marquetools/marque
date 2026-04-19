<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION TYPECHECK"?>
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00280">
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
		[ISM-ID-00280][Error] All @ism:displayOnlyTo attributes must be of type NmTokens. 
	</sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
	  	For all elements which contain an @ism:displayOnlyTo attribute, this rule ensures that the displayOnlyTo value matches the pattern
		defined for type NmTokens. 
	</sch:p>
	  <sch:rule id="ISM-ID-00280-R1" context="*[@ism:displayOnlyTo]">
		    <sch:assert test="util:meetsType(@ism:displayOnlyTo, $NmTokensPattern)" flag="error" role="error">
			[ISM-ID-00280][Error] All @ism:displayOnlyTo attributes values must be of type NmTokens. 
		</sch:assert>
	  </sch:rule>
</sch:pattern>