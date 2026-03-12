<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION TYPECHECK"?>
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00281">
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
		[ISM-ID-00281][Error] All @ism:disseminationControls attributes must be of type NmTokens. 
	</sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
	  	For all elements which contain a @ism:disseminationControls attribute, the disseminationControls value must match the pattern
		defined for type NmTokens. 
	</sch:p>
	  <sch:rule id="ISM-ID-00281-R1" context="*[@ism:disseminationControls]">
		    <sch:assert test="util:meetsType(@ism:disseminationControls, $NmTokensPattern)" flag="error" role="error">
		    	[ISM-ID-00281][Error] All @ism:disseminationControls attributes must be of type NmTokens.
		</sch:assert>
	  </sch:rule>
</sch:pattern>