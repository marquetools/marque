<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION TYPECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00295">
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
		[ISM-ID-00295][Error] All @ism:SARIdentifier attributes must be of type NmTokens. 
	</sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
	  	For all elements which contain an @ism:SARIdentifier attribute, this rule ensures that the SARIdentifier value matches the pattern
		defined for type NmTokens. 
	</sch:p>
	  <sch:rule id="ISM-ID-00295-R1" context="*[@ism:SARIdentifier]">
		    <sch:assert test="util:meetsType(@ism:SARIdentifier, $NmTokensPattern)" flag="error" role="error">
		    	[ISM-ID-00295][Error] All @ism:SARIdentifier attributes must be of type NmTokens. 
		</sch:assert>
	  </sch:rule>
</sch:pattern>