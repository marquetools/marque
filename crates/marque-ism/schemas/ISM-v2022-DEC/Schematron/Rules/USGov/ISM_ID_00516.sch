<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER TYPECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00516">
	<sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
	  	[ISM-ID-00516][Error] All @ism:secondBannerLine attributes must be of type NmTokens. 
	</sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
	  	For all elements which contain an @ism:secondBannerLine attribute, 
	  	this rule ensures that the secondBannerLine value matches the pattern defined for type NmTokens.  
	</sch:p>
	<sch:rule id="ISM-ID-00516-R1" context="*[@ism:secondBannerLine]">
		<sch:assert test="util:meetsType(@ism:secondBannerLine, $NmTokensPattern)" flag="error" role="error">
			[ISM-ID-00516][Error] All @ism:secondBannerLine attributes must be of type NmTokens. 
		</sch:assert>
	  </sch:rule>
</sch:pattern>