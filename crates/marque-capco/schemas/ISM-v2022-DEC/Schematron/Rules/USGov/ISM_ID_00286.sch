<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION TYPECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00286">
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
		[ISM-ID-00286][Error] All @ism:nonUSControls attributes must be of type NmTokens. 
	</sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
	  	For all elements which contain an @ism:nonUSControls attribute, this rule ensures that the nonUSControls value matches the pattern
		defined for type NmTokens. 
	</sch:p>
	  <sch:rule id="ISM-ID-00286-R1" context="*[@ism:nonUSControls]">
		    <sch:assert test="util:meetsType(@ism:nonUSControls, $NmTokensPattern)" flag="error" role="error">
		    	[ISM-ID-00286][Error] All @ism:nonUSControls attributes must be of type NmTokens. 
		</sch:assert>
	  </sch:rule>
</sch:pattern>