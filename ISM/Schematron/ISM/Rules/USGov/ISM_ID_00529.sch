<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00529">
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
	  	[ISM-ID-00529][Error] All tokens in the @ism:SARIdentifier attribute MUST conform to the regex 
	  	^SAR-[A-Z]{3,}:((C|S|TS):){0,1}[A-Za-z0-9._-]{1,}$ . Human Readable:  All tokens in @ism:SARIdentifier must conform to
	  	a regular expression for: SAR-SourceAuthority:Classification:SAPmarking or SAR-SourceAuthority:SAPmarking.
	</sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
	  	For all tokens within an @ism:SARIdentifier attribute, this rule ensures that each token follows the regex
	  	^SAR-[A-Z]{3,}:((C|S|TS):){0,1}[A-Za-z0-9._-]{1,}$
	</sch:p>
	<sch:rule id="ISM-ID-00529-R1" context="*[@ism:SARIdentifier]">
		<sch:let name="nonmatchingTokens" value="for $token in tokenize(normalize-space(string(@ism:SARIdentifier)), ' ') 
			return if (not(matches($token,'^SAR-[A-Z]{3,}:((C|S|TS):){0,1}[A-Za-z0-9._-]{1,}$'))) then $token else null"/>
		<sch:assert test="count($nonmatchingTokens) = 0" flag="error" 
			role="error">
			[ISM-ID-00529][Error] All tokens in the @ism:SARIdentifier attribute MUST conform to the regex 
			^SAR-[A-Z]{3,}:((C|S|TS):){0,1}[A-Za-z0-9._-]{1,}$ . Human Readable:  All tokens in @ism:SARIdentifier must conform to
			a regular expression for: SAR-SourceAuthority:Classification:SAPmarking or SAR-SourceAuthority:SAPmarking.
		</sch:assert>
	  </sch:rule>
</sch:pattern>