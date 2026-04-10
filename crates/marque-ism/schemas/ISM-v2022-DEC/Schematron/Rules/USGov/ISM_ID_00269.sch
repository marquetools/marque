<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION TYPECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00269">
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
		[ISM-ID-00269][Error] All @ism:classification attributes must be of type NmToken. 
	</sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
	  	For all elements which contain an @ism:classification attribute, this rule ensures that the classification value matches the pattern
		defined for type NmTokens.  
	</sch:p>
	  <sch:rule id="ISM-ID-00269-R1" context="*[@ism:classification]">
		    <sch:assert test="util:meetsType(@ism:classification, $NmTokenPattern)" flag="error" role="error">
		    	[ISM-ID-00269][Error] All @ism:classification attributes must be of type NmToken. 
		</sch:assert>
	  </sch:rule>
</sch:pattern>